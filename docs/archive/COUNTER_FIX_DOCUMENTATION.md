# Gameover Counter Animation Fix - Complete Analysis

## Problem Statement
Gameover scene counters (distance, score, coins) remained stuck at 0 and never animated, despite the infrastructure being in place for animation.

## Root Cause Analysis

### Issue #1: Panicking Variable Retrieval
**Code before fix:**
```rust
let target_score = c.get_i32("last_score").max(0);
let target_coins = c.get_i32("last_coins").max(0);
```

**Problem:** The Quartz Canvas API's `get_i32()` and `get_f32()` methods panic if the variable doesn't exist:
```rust
pub fn get_i32(&self, name: &str) -> i32 {
    match self.game_vars.get(name) {
        Some(Value::I32(v)) => *v,
        None => panic!("game_var '{name}' expected I32 but key was missing"),  // PANIC!
    }
}
```

**Impact:** If variables weren't set before scene transition, the initialization would crash. Even if variables existed, this is an unsafe pattern.

### Issue #2: Global Registration Flag Blocking Multi-Scene Support
**Code before fix:**
```rust
let registered = matches!(c.get_var("go_countup_registered"), Some(Value::Bool(true)));
if registered { return; }
c.on_update(|canvas| { ... });  // Register callback
c.set_var("go_countup_registered", true);  // Set GLOBAL flag
```

**Problem:** The game has THREE different gameover scenes:
1. gameover - for falling
2. gameover_sun - for flying into the sun
3. gameover_oxygen - for running out of oxygen

**Scenario that caused the bug:**
1. Player dies (falls) → gameover scene loads
2. init_gameover_countup() called with "go_stats_text" object ID
3. Global flag "go_countup_registered" set to true
4. Callback registered for gameover scene ✓
5. Player retries, falls differently into oxygen zone → dies
6. Oxygen death triggers → gameover_oxygen scene loads
7. init_gameover_countup() called with "oxy_go_stats_text" object ID
8. **Check: is "go_countup_registered" == true? YES → return early** ❌
9. **Second callback NEVER gets registered for gameover_oxygen scene** ❌
10. User sees counters stuck at 0 on gameover_oxygen

## Solution

### Fix #1: Safe Variable Retrieval Pattern
**Code after fix:**
```rust
let target_score = match c.get_var("last_score") {
    Some(Value::I32(v)) => v.max(0),
    _ => 0  // Safe default if missing or wrong type
};
```

**Benefits:**
- No panic on missing variables
- Graceful fallback to 0
- Handles both missing variables and type mismatches
- Applied to all three target variable retrievals (distance, score, coins)

### Fix #2: Per-Scene Registration Flags
**Code after fix:**
```rust
let flag_key = format!("go_countup_registered_{}", stats_object_id);
let registered = matches!(c.get_var(&flag_key), Some(Value::Bool(true)));
if registered { return; }

c.on_update(|canvas| { ... });  // Register callback
c.set_var(&flag_key, true);      // Set PER-SCENE flag
```

**How it works:**
- Each scene gets a unique flag based on its stats_object_id
- gameover scene → flag "go_countup_registered_go_stats_text"
- gameover_sun scene → flag "go_countup_registered_sun_go_stats_text"
- gameover_oxygen scene → flag "go_countup_registered_oxy_go_stats_text"
- Each scene can now register its own callback independently
- Prevents double-registration on revisiting same scene
- Allows other scenes to register without collision

## Files Modified
1. **src/menu.rs** - `init_gameover_countup()` function:
   - Safe variable retrieval (3 separate match statements)
   - Per-scene flag implementation (lines 154-161)
   - Applies to both initialization and on_update callback

2. **src/scenes/game/build_scene.rs** - death trigger (unchanged):
   - Still correctly sets "last_distance", "last_score", "last_coins" on Canvas
   - Variables properly persist to new scene via Canvas global state

## Verification

### Code Verification
- ✅ init_gameover_countup() called from all three scenes:
  - Line 1536: gameover scene → "go_stats_text"
  - Line 1722: gameover_sun scene → "sun_go_stats_text"
  - Line 1899: gameover_oxygen scene → "oxy_go_stats_text"

- ✅ All three scene IDs are unique
- ✅ All three match is_scene() checks in on_update callback
- ✅ Per-scene flags prevent collisions
- ✅ Safe variable retrieval prevents panics

### Build Verification
- ✅ cargo check: Finished (no errors)
- ✅ cargo build: Finished (no errors)
- ✅ Test suite: All validations passed

## Expected Behavior After Fix

**Scenario: Player dies three times in different hazards**

1. First death (fall):
   - Scene: gameover, stats_object_id: "go_stats_text"
   - Flag check: "go_countup_registered_go_stats_text" → not found
   - Action: Register on_update, set flag ✓
   - Animation: Counters count up from 0 ✓

2. Retry, second death (sun):
   - Scene: gameover_sun, stats_object_id: "sun_go_stats_text"
   - Flag check: "go_countup_registered_sun_go_stats_text" → not found (different key!)
   - Action: Register NEW on_update, set NEW flag ✓
   - Animation: Counters count up from 0 ✓

3. Retry, third death (oxygen):
   - Scene: gameover_oxygen, stats_object_id: "oxy_go_stats_text"
   - Flag check: "go_countup_registered_oxy_go_stats_text" → not found (different key!)
   - Action: Register NEW on_update, set NEW flag ✓
   - Animation: Counters count up from 0 ✓

All three scenes now properly animate their counters with no interference.

## Counter Animation Details
The counter animation itself (implemented in gameover_step_countup) is unchanged and works correctly:
- Starts at 1 (not 0) for visual impact
- Uses tier-based minimum steps (1/2/4/8 based on magnitude)
- Accelerates with 1.08× multiplier per frame
- Capped at remaining/18 to prevent overshooting
- Smooth digit-by-digit growth visible to player
