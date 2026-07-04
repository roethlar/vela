# rev-6: DECLINED at intake

**Candidate** (coder-originated): `listings-updated` triggers `resetAndLoad()`,
which resets scroll position and pagination while the user is mid-browse.

**Why declined**: the event only fires when a background revalidation found
the directory's contents actually changed; refreshing the view then is the
designed stale-while-revalidate semantic, and the listing genuinely differs
from what the scroll position indexed into. The observable cost (a rare
scroll reset, only on real change) does not justify patch-in-place complexity
now. Reopen if playtests show revalidation churn making it frequent.
