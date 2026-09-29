ml_stream_declare and ml_stream_advance, as the mlos-stream crate. A stream is a declared CYCLIC order plus a cursor, and it is what finally writes ObjectMeta::next_use -- the field that existed since M2 step 001 with nothing to set it.

Proved in a running guest:
  mlsh> stream           -> declared 128 objects, cursor at 0
  mlsh> get 3 7
  mlsh> objs             -> L03 T07  Warm  0x4023e410  used 1  next at 55
(55 = layer 3 * 16 + tensor 7.)

THE CONSTRAINT FORCED AN ABI CHANGE, and this is the step's main finding. 'Advancing must not walk the whole table' is unsatisfiable while NextUse holds a DISTANCE: a distance is measured from somewhere, so moving the cursor one step invalidates every resident object's distance and the kernel must walk the table to fix them -- a per-token cost over the very structure it walks. NextUse::Distance -> NextUse::At, a POSITION, which does not move when the cursor does. advance() is one addition; the subtraction happens once, in the policy, for the few objects it compares. Residency gained now() so a policy can do it while still only reading what the table owns (design.md s.2).

THE SIMULATOR CHANGED THE SAME WAY AND GOT SIMPLER: it used to rewrite every resident object's distance on EVERY access (O(resident) per access); it now writes one position per acquire. foresee() is gone, replaced by wanted_at().

NEUTRALITY PROVEN: the verdict's numbers are unchanged -- 12,599 at 128 KiB, 3,480 at 192 KiB, identical to step 006. Writing the numbers down before refactoring is what made that checkable.

A BUG FOUND BY RUNNING IT, NOT BY A TEST. objs showed 'L00 T00 next at 0' after advancing the cursor to 40: the object sitting exactly ON the cursor was told its next use is HERE, permanently. Position never changes, cursor runs past, policy computes at-now, saturates to 0, and pins exactly the object it should evict first. next_after() must answer STRICTLY AFTER the cursor -- the use happening now is not the one a policy needs. Regression test in mlos-stream/tests/declared.rs.

DESIGN: a stream declares ONE PERIOD (cap 256 ids), not an enumeration -- every token reads the same objects in the same order, and declaring a thousand tokens would store the same ids a thousand times. next_after() scans one period per ACQUIRE, which is bounded by the declaration rather than by the object table; an acquire already costs a table lookup at best and a fetch at worst.

NextUse::Probability is still produced by nothing, deliberately: a declared stream says exactly WHEN, only a router says how LIKELY, and nothing routes until M5. Preserved rather than collapsed.

No userspace, as planned: declare() derives the order from model::tile, which is 'told' in exactly the sense a process would tell it.

mlsh gained a 5th module (acquire.rs) rather than leaving objects.rs at exactly 7 functions -- AGENTS.md calls that a failure waiting for the next one-line change.

sw-checklist 181/0/5 -- same five as before the step.