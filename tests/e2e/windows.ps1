param(
    [Parameter(Mandatory)][ValidateSet('config-location', 'processes', 'process-name', 'process-text', 'place-window', 'screenshot', 'private-file', 'protect-fixtures', 'trust-ca', 'untrust-ca', 'wait-clients')][string]$Action,
    [string]$Value,
    [string]$Detail
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This helper requires Windows.' }

function Write-Json($ValueToWrite) { ConvertTo-Json -InputObject $ValueToWrite -Depth 5 -Compress }
function Config-Directory {
    Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'vela/vela/config'
}

switch ($Action) {
    'config-location' {
        # Same known folder as directories::ProjectDirs; APPDATA is not an override.
        Write-Json @{ roaming = [Environment]::GetFolderPath('ApplicationData'); running = @(Get-Process vela -ErrorAction SilentlyContinue | ForEach-Object Id) }
    }
    'processes' {
        Write-Json @(Get-CimInstance Win32_Process | ForEach-Object {
            @{ pid = [int]$_.ProcessId; parent = [int]$_.ParentProcessId; exe = $_.ExecutablePath; cmdline = $_.CommandLine }
        })
    }
    'process-name' { Write-Json (Get-Process -Id ([int]$Value)).ProcessName.ToLowerInvariant() }
    'process-text' {
        if ($Detail -eq 'cmdline') {
            $processInfo = Get-CimInstance Win32_Process -Filter "ProcessId = $([int]$Value)"
            if (-not $processInfo.CommandLine) { throw 'Could not read the player command line.' }
            Write-Json $processInfo.CommandLine
        } elseif ($Detail -eq 'environ') {
            Add-Type -Path (Join-Path $PSScriptRoot 'windows-native.cs')
            Write-Json ([VelaE2E.Native]::EnvironmentBlock([int]$Value))
        } else { throw "Unknown process text: $Detail" }
    }
    'place-window' {
        Add-Type -Path (Join-Path $PSScriptRoot 'windows-native.cs')
        [VelaE2E.Native]::PlaceWindow([int]$Value)
        Write-Json $true
    }
    'screenshot' {
        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
        Add-Type -Path (Join-Path $PSScriptRoot 'windows-native.cs')
        [VelaE2E.Native]::SetProcessDPIAware() | Out-Null
        $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $bitmap = [System.Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bitmap.Size)
            $bitmap.Save($Value, [System.Drawing.Imaging.ImageFormat]::Png)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
        Write-Json @{ width = $bounds.Width; height = $bounds.Height }
    }
    'protect-fixtures' {
        # The seeded mode:0600 files need the Windows equivalent before Vela
        # reads them. Match storage.rs: protected DACL, owner + SYSTEM + admins.
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        foreach ($file in Get-ChildItem -LiteralPath (Config-Directory) -Recurse -File) {
            $acl = [System.Security.AccessControl.FileSecurity]::new()
            $acl.SetSecurityDescriptorSddlForm("D:P(A;;FA;;;$sid)(A;;FA;;;SY)(A;;FA;;;BA)")
            Set-Acl -LiteralPath $file.FullName -AclObject $acl
        }
        Write-Json $true
    }
    'trust-ca' {
        # SChannel, which Vela's HTTPS client uses on Windows, ignores
        # SSL_CERT_FILE: a scenario's mock CA is trusted in the machine's root
        # store, on a disposable runner only, and removed after the scenario.
        if ($env:VELA_E2E_DISPOSABLE_WINDOWS -ne '1') {
            throw 'Trusting a test CA changes the machine; set VELA_E2E_DISPOSABLE_WINDOWS=1 on a disposable Windows account.'
        }
        $cert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($Value)
        $store = [System.Security.Cryptography.X509Certificates.X509Store]::new('Root', 'LocalMachine')
        $store.Open('ReadWrite')
        try { $store.Add($cert) } finally { $store.Close() }
        Write-Json @{ thumbprint = $cert.Thumbprint }
    }
    'untrust-ca' {
        if ($Value -notmatch '^[0-9A-F]{40}$') { throw 'Expected a certificate thumbprint.' }
        $store = [System.Security.Cryptography.X509Certificates.X509Store]::new('Root', 'LocalMachine')
        $store.Open('ReadWrite')
        try {
            foreach ($cert in @($store.Certificates.Find('FindByThumbprint', $Value, $false))) { $store.Remove($cert) }
        } finally { $store.Close() }
        Write-Json @{ removed = $Value }
    }
    'private-file' {
        $acl = Get-Acl -LiteralPath $Value
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        $expected = @($sid, 'S-1-5-18', 'S-1-5-32-544')
        $rules = @($acl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]))
        $valid = $acl.AreAccessRulesProtected -and $rules.Count -eq 3
        foreach ($rule in $rules) {
            $valid = $valid -and $rule.IdentityReference.Value -in $expected -and
                $rule.AccessControlType -eq 'Allow' -and -not $rule.IsInherited -and
                $rule.FileSystemRights -eq [System.Security.AccessControl.FileSystemRights]::FullControl
        }
        $valid = $valid -and (@($rules | ForEach-Object { $_.IdentityReference.Value } | Sort-Object -Unique).Count -eq 3)
        Write-Json ([bool]$valid)
    }
    'wait-clients' {
        if (-not $Value.StartsWith('\\.\pipe\')) { throw 'Expected an mpv named pipe.' }
        $pipe = [System.IO.Pipes.NamedPipeClientStream]::new('.', $Value.Substring(9), [System.IO.Pipes.PipeDirection]::InOut)
        try {
            $pipe.Connect(10000)
            $deadline = [DateTime]::UtcNow.AddSeconds(15)
            do {
                # mpv input/ipc-win.c keeps one listening instance alongside
                # its connected instances. Subtract it AND this probe. If the
                # listener is momentarily absent this undercounts, never passes early.
                $accepted = $pipe.NumberOfServerInstances - 2
                if ($accepted -ge [int]$Detail) { Write-Json $accepted; return }
                Start-Sleep -Milliseconds 20
            } while ([DateTime]::UtcNow -lt $deadline)
            throw "mpv accepted $accepted clients, wanted $Detail (excluding this probe and the listener)"
        } finally { $pipe.Dispose() }
    }
}
