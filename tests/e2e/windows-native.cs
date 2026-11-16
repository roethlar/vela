// Read-only process evidence and desktop placement for the x64 Windows venue.
// Process environment reads fail closed; an unreadable block is never an
// empty block that would make the token-leak assertion pass.
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace VelaE2E {
    public static class Native {
        [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
        [DllImport("user32.dll", SetLastError = true)] static extern bool MoveWindow(IntPtr h, int x, int y, int w, int height, bool repaint);
        [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h, int command);
        [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
        [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
        [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool IsWow64Process(IntPtr h, out bool wow64);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool ReadProcessMemory(IntPtr h, IntPtr address, byte[] buffer, UIntPtr size, out UIntPtr read);
        [DllImport("ntdll.dll")] static extern int NtQueryInformationProcess(IntPtr h, int kind, [Out] IntPtr[] info, int size, out int returned);

        public static void PlaceWindow(int pid) {
            SetProcessDPIAware();
            var window = Process.GetProcessById(pid).MainWindowHandle;
            if (window == IntPtr.Zero) throw new Exception("Vela has no main window");
            ShowWindow(window, 9); // SW_RESTORE. The hosted desktop is 1024x768.
            if (!MoveWindow(window, 0, 0, 1000, 700, true)) throw new Win32Exception();
            SetForegroundWindow(window);
        }

        static byte[] Read(IntPtr process, long address, int length) {
            var bytes = new byte[length];
            UIntPtr read;
            if (!ReadProcessMemory(process, new IntPtr(address), bytes, (UIntPtr)length, out read) || read.ToUInt64() != (ulong)length)
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Cannot read the player's process memory");
            return bytes;
        }

        public static string EnvironmentBlock(int pid) {
            if (IntPtr.Size != 8) throw new Exception("The Windows E2E venue requires x64 PowerShell and Vela");
            var process = OpenProcess(0x0400 | 0x0010, false, pid); // QUERY_INFORMATION | VM_READ
            if (process == IntPtr.Zero) throw new Win32Exception();
            try {
                bool wow64;
                if (!IsWow64Process(process, out wow64) || wow64) throw new Exception("Expected an x64 player process");
                var basic = new IntPtr[6];
                int returned;
                if (NtQueryInformationProcess(process, 0, basic, 48, out returned) != 0 || returned != 48)
                    throw new Exception("Cannot read PROCESS_BASIC_INFORMATION");
                // Native x64 PEB.ProcessParameters (+0x20) and
                // RTL_USER_PROCESS_PARAMETERS.Environment (+0x80).
                long parameters = BitConverter.ToInt64(Read(process, basic[1].ToInt64() + 0x20, 8), 0);
                long environment = BitConverter.ToInt64(Read(process, parameters + 0x80, 8), 0);
                if (environment == 0) throw new Exception("Player environment pointer is null");
                var text = new StringBuilder();
                // Read at page boundaries so the last chunk cannot cross into
                // an unmapped page beyond the environment's allocation.
                for (int offset = 0; offset < 16 * 1024 * 1024;) {
                    int count = (int)(4096 - ((environment + offset) & 4095));
                    var chunk = Encoding.Unicode.GetString(Read(process, environment + offset, count));
                    foreach (char ch in chunk) {
                        if (ch == '\0' && text.Length > 0 && text[text.Length - 1] == '\0') return text.ToString();
                        text.Append(ch);
                    }
                    offset += count;
                }
                throw new Exception("Player environment has no terminator within 16 MiB");
            } finally { CloseHandle(process); }
        }
    }
}
