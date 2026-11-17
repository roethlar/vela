//! Privacy policy after Win32 has resolved trustees by SID equality.
//! No SID spelling (including the LA/LG aliases for RID 500/501) reaches here.

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Ace {
    pub access_allowed: bool,
    // These can overlap: a process running as LocalSystem has that user SID.
    pub current_user: bool,
    pub system: bool,
    pub administrators: bool,
}

pub(super) fn is_private(protected: bool, aces: &[Ace]) -> bool {
    if !protected {
        return false;
    }
    let mut user = false;
    let mut system = false;
    let mut administrators = false;
    for ace in aces {
        // Deny, callback, object-specific and all other ACE types need
        // semantics this policy does not interpret, so reject them all.
        if !ace.access_allowed {
            return false;
        }
        // Everyone, Users, Authenticated Users, and any unrelated identity
        // all arrive with no match. Never make exceptions for an extra SID.
        if !ace.current_user && !ace.system && !ace.administrators {
            return false;
        }
        user |= ace.current_user;
        system |= ace.system;
        administrators |= ace.administrators;
    }
    // An absent/NULL DACL is rejected by the reader; an empty ACL fails here.
    user && system && administrators
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_aces() -> Vec<Ace> {
        vec![
            Ace {
                access_allowed: true,
                current_user: true,
                ..Ace::default()
            },
            Ace {
                access_allowed: true,
                system: true,
                ..Ace::default()
            },
            Ace {
                access_allowed: true,
                administrators: true,
                ..Ace::default()
            },
        ]
    }

    #[test]
    fn matching_user_sid_accepts_rid_500_and_501_regardless_of_sddl_alias() {
        // W2: S-1-5-21-3289802278-1561746829-1498425070-500 was
        // rendered as LA. LG is the same issue for RID 501. In either case
        // EqualSid yields current_user=true, exactly as for any other user.
        // SDDL text is deliberately not an input to the policy.
        assert!(is_private(true, &private_aces()));
    }

    #[test]
    fn unprotected_dacl_is_rejected() {
        assert!(!is_private(false, &private_aces()));
    }

    #[test]
    fn each_required_trustee_must_be_present() {
        for missing in 0..3 {
            let mut aces = private_aces();
            aces.remove(missing);
            assert!(!is_private(true, &aces), "missing trustee {missing}");
        }
        assert!(!is_private(true, &[]));
    }

    #[test]
    fn any_extra_trustee_is_rejected() {
        // The Win32 reader maps all of these to the same no-match value.
        for trustee in ["Everyone", "Users", "Authenticated Users", "unrelated user"] {
            let mut aces = private_aces();
            aces.push(Ace {
                access_allowed: true,
                ..Ace::default()
            });
            assert!(!is_private(true, &aces), "extra trustee: {trustee}");
        }
    }

    #[test]
    fn non_allow_aces_are_rejected_even_for_permitted_trustees() {
        for trustee in private_aces() {
            let mut aces = private_aces();
            aces.push(Ace {
                access_allowed: false,
                ..trustee
            });
            assert!(!is_private(true, &aces));
        }
    }

    #[test]
    fn permitted_duplicates_and_order_do_not_change_privacy() {
        let mut aces = private_aces();
        aces.extend(private_aces());
        aces.reverse();
        assert!(is_private(true, &aces));
    }

    #[test]
    fn current_user_can_also_be_system() {
        let aces = [
            Ace {
                access_allowed: true,
                current_user: true,
                system: true,
                ..Ace::default()
            },
            Ace {
                access_allowed: true,
                administrators: true,
                ..Ace::default()
            },
        ];
        assert!(is_private(true, &aces));
    }
}
