# Quartz Game Engine — Complete Developer Tutorial

> Build 2D games in Rust with scenes, physics, cameras, and animated sprites.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Project Setup](#2-project-setup)
3. [The Canvas](#3-the-canvas)
4. [Game Objects](#4-game-objects)
5. [The Targeting System](#5-the-targeting-system)
6. [Events](#6-events)
7. [Actions](#7-actions)
8. [Conditions](#8-conditions)
9. [Locations](#9-locations)
10. [Scenes](#10-scenes)
11. [Camera](#11-camera)
12. [Animated Sprites](#12-animated-sprites)
13. [Physics Deep Dive](#13-physics-deep-dive)
14. [Custom Events](#14-custom-events)
15. [Infinite Scrolling](#15-infinite-scrolling)
16. [Sound](#16-sound)
17. [Complete Example — Grass World Platformer](#17-complete-example--grass-world-platformer)
18. [Common Game Recipes](#18-common-game-recipes)
19. [Quick Reference Card](#19-quick-reference-card)

---

## 1. Overview

Quartz is a Rust 2D game engine built on the **Prism** rendering framework. It gives you a high-level, declarative API so you can focus entirely on game logic instead of low-level rendering plumbing.

**What Quartz handles for you:**
- A virtual canvas that auto-scales to any real screen resolution
- Game objects with built-in physics — gravity, momentum, and resistance
- AABB collision detection, including solid platform landing
- A declarative event system for keyboard input, ticks, and collisions
- Scene management to separate game states (menus, levels, game-over screens)
- A smooth lerp-following camera with world-boundary clamping
- GIF-based animated sprites with per-frame timing control

### Core mental model

Everything in Quartz follows a simple structure:

```
Canvas
 └─ GameObjects   (things that exist in the world)
     └─ Events    (triggers: key press, tick, collision…)
         └─ Actions  (what to do: move, spawn, hide, teleport…)
```

You build a world by creating objects, wiring events to actions, and letting the engine run the game loop for you. The engine ticks at ~60 fps and handles physics, collisions, and event dispatch automatically.

---

## 2. Project Setup

### Cargo.toml

```toml
[dependencies]
quartz = { path = "../quartz" }
ramp   = { path = "../ramp" }
image  = "0.25"
```

### Entry point

Every Quartz game uses the `ramp::run!` macro. It receives a `Context` and an `Assets` handle, and must return something that implements `Drawable`. `Canvas` implements `Drawable`, so you typically return a configured canvas directly.

```rust
use quartz::{Canvas, CanvasMode, Context};
use ramp::prism;
use prism::drawable::Drawable;

ramp::run! { |ctx: &mut Context, _assets: Assets| {
    MyGame::new(ctx)
}}
```

### Recommended file layout

```
src/
  main.rs            ← ramp::run! entry point
  game.rs            ← Canvas setup, scene registration
  scenes/
    level1.rs        ← scene builder functions
    menu.rs
    game_over.rs
assets/
  run.gif
  idle.gif
  jump.gif
  coin.wav
  music.mp3
```

---

## 3. The Canvas

The `Canvas` is the root of your entire game. It owns all game objects, manages events, controls the camera, and handles scene transitions. It renders everything at a fixed **virtual resolution** and automatically scales to fill the window — you never need to think about the real screen size.

### Canvas modes

| Mode | Virtual Resolution | Aspect Ratio | Use for |
|---|---|---|---|
| `CanvasMode::Landscape` | 3840 × 2160 | 16:9 | Side-scrollers, top-down games |
| `CanvasMode::Portrait` | 2160 × 3840 | 9:16 | Mobile-style vertical games |

All positions and sizes you write are in **virtual pixels**. A player that is 100px wide on a 3840-wide canvas looks correct on any screen.

### Creating a canvas

```rust
let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
```

### Key canvas methods

| Method | Description |
|---|---|
| `Canvas::new(ctx, mode)` | Create a new canvas |
| `canvas.add_game_object(name, obj)` | Add a `GameObject` with a string name |
| `canvas.remove_game_object(name)` | Remove an object by name |
| `canvas.get_game_object(name)` | Returns `Option<&GameObject>` |
| `canvas.get_game_object_mut(name)` | Returns `Option<&mut GameObject>` |
| `canvas.add_event(event, target)` | Register a `GameEvent` on a target |
| `canvas.run(action)` | Execute an `Action` immediately |
| `canvas.on_tick(cb)` | Register a per-frame closure |
| `canvas.on_key_press(cb)` | Register a key-press closure |
| `canvas.on_key_release(cb)` | Register a key-release closure |
| `canvas.is_key_held(key)` | Returns `bool` — is this key currently held? |
| `canvas.get_virtual_size()` | Returns `(f32, f32)` — the virtual resolution |
| `canvas.collision_between(t1, t2)` | Returns `bool` — are these targets overlapping? |
| `canvas.objects_in_radius(obj, px)` | Returns all visible objects within a pixel radius |
| `canvas.play_sound(path)` | Play a sound file asynchronously |
| `canvas.set_camera(camera)` | Attach a `Camera` |
| `canvas.clear_camera()` | Remove the active camera |
| `canvas.camera()` | `Option<&Camera>` |
| `canvas.camera_mut()` | `Option<&mut Camera>` |
| `canvas.add_scene(scene)` | Register a scene |
| `canvas.load_scene(name)` | Transition to a scene |
| `canvas.active_scene()` | `Option<&str>` — name of the current scene |
| `canvas.is_scene(name)` | `bool` — is this the active scene? |
| `canvas.register_custom_event(name, cb)` | Register a named custom handler |

---

## 4. Game Objects

A `GameObject` is a rectangle in the world. It has a position, a size, an optional image or animated sprite, and physics properties. Every visible entity in your game — players, enemies, platforms, backgrounds, coins, bullets, UI elements — is a `GameObject`.

### Creating a square object

`GameObject::new` takes a single `f32` for size, making width and height equal.

```rust
use quartz::{Image, ShapeType};

// Build a solid red square
let image = Image {
    shape: ShapeType::Rectangle(0.0, (100.0, 100.0), 0.0),
    image: image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255])).into(),
    color: None,
};

let player = GameObject::new(
    ctx,
    "player".to_string(),  // id — used for lookups
    Some(image),            // optional image (None = invisible)
    100.0,                  // size (square — single f32)
    (300.0, 800.0),         // starting position (x, y) — top-left corner
    vec![],                 // tags for group targeting
    (0.0, 0.0),             // initial momentum (vx, vy)
    (0.85, 1.0),            // resistance — 0.85 horizontal friction, no vertical damping
    0.55,                   // gravity added to vy each tick
);
```

### Creating a rectangular object

Use `GameObject::new_rect` for non-square shapes like platforms, wide backgrounds, and UI panels.

```rust
let platform = GameObject::new_rect(
    ctx,
    "platform_1".to_string(),
    Some(image),
    (500.0, 30.0),    // (width, height)
    (600.0, 1200.0),  // position
    vec![],
    (0.0, 0.0),
    (1.0, 1.0),       // no friction (static object)
    0.0,              // no gravity
).as_platform();      // mark as a solid landing surface
```

### Builder methods — chainable after new or new_rect

```rust
let enemy = GameObject::new_rect(ctx, "enemy_1".to_string(), Some(img),
    (80.0, 120.0), (1000.0, 900.0), vec![], (0.0, 0.0), (0.9, 1.0), 0.55)
    .with_tag("enemy")           // add to the "enemy" group
    .with_tag("patroller")       // multiple tags are allowed
    .with_gravity(0.6)           // override gravity
    .with_momentum((-2.0, 0.0)); // start moving left immediately
```

| Builder method | Effect |
|---|---|
| `.as_platform()` | Objects with downward momentum land on top of this |
| `.with_tag("name")` | Add a single tag |
| `.with_tags(vec![...])` | Replace the entire tag list |
| `.with_gravity(f)` | Set gravity (`momentum.y += gravity` each tick) |
| `.with_momentum((vx, vy))` | Set initial velocity |
| `.with_resistance((rx, ry))` | Per-axis multiplier applied to momentum each tick |
| `.with_image(image)` | Attach a static image |
| `.with_animation(sprite)` | Attach a GIF `AnimatedSprite` (overrides static image) |

### Mutating an object after creation

```rust
if let Some(player) = canvas.get_game_object_mut("player") {
    player.momentum = (10.0, -22.0);  // set velocity directly
    player.gravity  = 0.8;
    player.visible  = false;          // hide the object
}
```

### Public fields reference

| Field | Type | Description |
|---|---|---|
| `id` | `String` | Unique identifier string |
| `tags` | `Vec<String>` | Tags for group targeting |
| `size` | `(f32, f32)` | Width × height in virtual pixels |
| `position` | `(f32, f32)` | Top-left corner in world coordinates |
| `momentum` | `(f32, f32)` | Velocity added to position each tick |
| `resistance` | `(f32, f32)` | Per-axis multiplier on momentum each tick (0.0–1.0) |
| `gravity` | `f32` | Added to `momentum.y` each tick |
| `is_platform` | `bool` | Whether other objects land on top of this |
| `visible` | `bool` | Hidden objects skip rendering and collision checks |
| `animated_sprite` | `Option<AnimatedSprite>` | Active sprite animation |

### Adding and removing objects at runtime

You can add and remove objects at any time — from tick callbacks, event handlers, or custom events:

```rust
// Spawn a new enemy mid-game
canvas.add_game_object("enemy_wave2_0".to_string(), new_enemy);

// Remove a collected coin
canvas.remove_game_object("coin_7");
```

---

## 5. The Targeting System

Almost every action and event needs a `Target` that identifies which object(s) to affect. There are three modes:

| Variant | Constructor shorthand | Matches |
|---|---|---|
| `Target::ByName(name)` | `Target::name("player")` | Exactly one object by its registered name |
| `Target::ById(id)` | `Target::id("abc-123")` | Exactly one object by its `id` field |
| `Target::ByTag(tag)` | `Target::tag("enemy")` | **All** objects sharing the tag |

### Why tags are powerful

Tags let you apply one action to an entire group at once:

```rust
// Tag every enemy at creation
let enemy = GameObject::new_rect(...)
    .with_tag("enemy");

// Later — freeze all enemies at once
canvas.run(Action::SetMomentum {
    target: Target::tag("enemy"),
    value: (0.0, 0.0),
});

// Check if the player is touching any coin
if canvas.collision_between(
    &Target::name("player"),
    &Target::tag("coin"),
) {
    // player touched a coin!
}
```

---

## 6. Events

Events connect a **trigger** to an **Action**. The trigger can be a key press, every game tick, a collision, or a custom signal. You register events with:

```rust
canvas.add_event(event, owner_target);
```

The second argument — `owner_target` — determines which object's event list this goes into. For most cases you'll pass the same target as inside the event struct.

### KeyPress — fires once when the key is first pressed

```rust
canvas.add_event(
    GameEvent::KeyPress {
        key: Key::Named(NamedKey::ArrowUp),
        action: Action::ApplyMomentum {
            target: Target::name("player"),
            value: (0.0, -22.0),   // jump!
        },
        target: Target::name("player"),
    },
    Target::name("player"),
);
```

### KeyHold — fires every tick while the key is held

```rust
canvas.add_event(
    GameEvent::KeyHold {
        key: Key::Named(NamedKey::ArrowRight),
        action: Action::ApplyMomentum {
            target: Target::name("player"),
            value: (5.0, 0.0),
        },
        target: Target::name("player"),
    },
    Target::name("player"),
);
```

### KeyRelease — fires once when the key is released

```rust
canvas.add_event(
    GameEvent::KeyRelease {
        key: Key::Named(NamedKey::Space),
        action: Action::Custom { name: "fire_released".to_string() },
        target: Target::name("player"),
    },
    Target::name("player"),
);
```

### Tick — fires every frame (~60 fps)

```rust
canvas.add_event(
    GameEvent::Tick {
        action: Action::ApplyMomentum {
            target: Target::name("cloud"),
            value: (-1.0, 0.0),   // drift left every tick
        },
        target: Target::name("cloud"),
    },
    Target::name("cloud"),
);
```

### Collision — fires when the owning object overlaps any other object

```rust
canvas.add_event(
    GameEvent::Collision {
        action: Action::Remove {
            target: Target::name("bullet"),
        },
        target: Target::name("bullet"),
    },
    Target::name("bullet"),
);
```

### BoundaryCollision — fires when the object hits the canvas edge

```rust
canvas.add_event(
    GameEvent::BoundaryCollision {
        action: Action::Remove {
            target: Target::tag("projectile"),
        },
        target: Target::tag("projectile"),
    },
    Target::tag("projectile"),
);
```

### Inline tick callbacks — for complex logic

When simple event/action wiring isn't enough, `on_tick` gives you full canvas access in a closure:

```rust
canvas.on_tick(|canvas| {
    // Check if the player fell off the world
    if let Some(player) = canvas.get_game_object("player") {
        if player.position.1 > 4000.0 {
            canvas.load_scene("game_over");
        }
    }
});
```

### Key press/release callbacks

For when you need to inspect which key was pressed:

```rust
canvas.on_key_press(|canvas, key| {
    if *key == Key::Named(NamedKey::Escape) {
        canvas.load_scene("pause_menu");
    }
});
```

### Checking key state from within a tick callback

```rust
canvas.on_tick(|canvas| {
    let moving = canvas.is_key_held(&Key::Named(NamedKey::ArrowRight))
              || canvas.is_key_held(&Key::Named(NamedKey::ArrowLeft));
    if !moving {
        // switch to idle animation
    }
});
```

### Common key values

```rust
Key::Named(NamedKey::ArrowLeft)
Key::Named(NamedKey::ArrowRight)
Key::Named(NamedKey::ArrowUp)
Key::Named(NamedKey::ArrowDown)
Key::Named(NamedKey::Space)
Key::Named(NamedKey::Enter)
Key::Named(NamedKey::Escape)
Key::Character("a".to_string())   // letter keys
Key::Character("w".to_string())
Key::Character("s".to_string())
Key::Character("d".to_string())
```

---

## 7. Actions

Actions are the **verbs** of Quartz — they describe what happens. You can embed them inside events, run them directly with `canvas.run(action)`, or nest them inside `Action::Conditional`.

### ApplyMomentum — add to current velocity

```rust
Action::ApplyMomentum {
    target: Target::name("player"),
    value: (5.0, 0.0),   // push right
}
```

Use this for movement and jumps. Each `KeyHold` call adds to momentum, which builds up speed naturally, capped by your resistance value.

### SetMomentum — set velocity directly

```rust
Action::SetMomentum {
    target: Target::name("enemy"),
    value: (-3.0, 0.0),   // move left at a fixed speed
}
```

Use when you want precise, predictable velocity rather than acceleration.

### SetResistance — change friction at runtime

```rust
Action::SetResistance {
    target: Target::name("player"),
    value: (0.5, 1.0),   // apply extra braking
}
```

### Teleport — instantly move to a location

```rust
Action::Teleport {
    target: Target::name("player"),
    location: Location::at(300.0, 900.0),
}
```

Great for respawning after death or placing a spawned object precisely.

### Spawn — create a new object in the world

```rust
Action::Spawn {
    object: Box::new(bullet_template.clone()),
    location: Location::OnTarget {
        target: Box::new(Target::name("player")),
        anchor: Anchor { x: 1.0, y: 0.5 },   // right-center edge of player
        offset: (10.0, 0.0),
    },
}
```

The spawned object gets the auto-generated name `spawned_<id>`.

### Remove — delete an object

```rust
Action::Remove { target: Target::name("coin_3") }
// Or remove a whole group:
Action::Remove { target: Target::tag("enemy") }
```

### Show / Hide / Toggle — control visibility

```rust
Action::Show   { target: Target::tag("ui_hud") }
Action::Hide   { target: Target::tag("ui_hud") }
Action::Toggle { target: Target::name("door") }
```

Hidden objects are not rendered and do not participate in collision detection.

### SetAnimation — swap the active sprite

```rust
// GIF bytes must be 'static — use include_bytes!
static RUN_GIF: &[u8] = include_bytes!("../assets/run.gif");

Action::SetAnimation {
    target: Target::name("player"),
    animation_bytes: RUN_GIF,
    fps: 12.0,
}
```

### TransferMomentum — copy velocity from one object to another

```rust
Action::TransferMomentum {
    from: Target::name("boulder"),
    to: Target::name("player"),
    scale: 0.5,   // player receives 50% of boulder's momentum
}
```

### Conditional — branch on a condition

```rust
Action::Conditional {
    condition: Condition::KeyHeld(Key::Named(NamedKey::ArrowRight)),
    if_true: Box::new(Action::ApplyMomentum {
        target: Target::name("player"),
        value: (5.0, 0.0),
    }),
    if_false: Some(Box::new(Action::SetMomentum {
        target: Target::name("player"),
        value: (0.0, 0.0),
    })),
}
```

`if_false` accepts `None` if you only want a one-sided branch.

### Custom — invoke a named handler

```rust
Action::Custom { name: "player_died".to_string() }
```

Pairs with `canvas.register_custom_event(...)`. See [section 14](#14-custom-events).

---

## 8. Conditions

Conditions are used inside `Action::Conditional`. They let you branch game logic on runtime state without needing a tick callback.

| Condition | True when... |
|---|---|
| `Condition::Always` | Always — unconditional branch |
| `Condition::KeyHeld(key)` | The key is currently held down |
| `Condition::KeyNotHeld(key)` | The key is NOT currently held |
| `Condition::Collision(target)` | Target is overlapping any other object |
| `Condition::NoCollision(target)` | Target is NOT overlapping anything |
| `Condition::IsVisible(target)` | Any matching object is visible |
| `Condition::IsHidden(target)` | Any matching object is hidden |
| `Condition::And(c1, c2)` | Both sub-conditions are true |
| `Condition::Or(c1, c2)` | Either sub-condition is true |
| `Condition::Not(c)` | The sub-condition is false |

### Composing conditions

```rust
// Only jump if player is grounded (currently colliding with a platform)
Action::Conditional {
    condition: Condition::And(
        Box::new(Condition::KeyHeld(Key::Named(NamedKey::Space))),
        Box::new(Condition::Collision(Target::name("player"))),
    ),
    if_true: Box::new(Action::ApplyMomentum {
        target: Target::name("player"),
        value: (0.0, -22.0),
    }),
    if_false: None,
}
```

---

## 9. Locations

Locations are used by `Action::Teleport` and `Action::Spawn` to express *where* something should go. They resolve to a world-space `(x, y)` position at the moment the action runs.

### `Location::at(x, y)` — fixed coordinate

```rust
Location::at(500.0, 900.0)
```

### `Location::AtTarget(target)` — match another object's position

```rust
Location::AtTarget(Box::new(Target::name("spawn_point")))
```

### `Location::Relative { target, offset }` — offset from another object

```rust
Location::Relative {
    target: Box::new(Target::name("player")),
    offset: (0.0, -150.0),   // 150px directly above the player
}
```

### `Location::OnTarget { target, anchor, offset }` — anchor point on another object

Anchor uses **normalized coordinates**: `(0.0, 0.0)` is top-left, `(1.0, 1.0)` is bottom-right.

```rust
// Spawn a bullet from the right-center edge of the player
Location::OnTarget {
    target: Box::new(Target::name("player")),
    anchor: Anchor { x: 1.0, y: 0.5 },
    offset: (5.0, 0.0),
}
```

### `Location::Between(t1, t2)` — midpoint between two objects

```rust
Location::Between(
    Box::new(Target::name("object_a")),
    Box::new(Target::name("object_b")),
)
```

---

## 10. Scenes

Scenes split your game into discrete states: a main menu, multiple levels, a pause screen, a game-over screen. Each scene has its own objects and events. Switching scenes removes the old objects and adds the new ones automatically.

### Building a scene

Use the builder pattern to chain objects and events:

```rust
fn build_level_1(ctx: &mut Context) -> Scene {
    let background = /* ... */;
    let ground     = /* ... */;
    let player     = /* ... */;

    Scene::new("level_1")
        .with_object("background", background)
        .with_object("ground",     ground)
        .with_object("player",     player)
        .with_event(
            GameEvent::KeyHold {
                key: Key::Named(NamedKey::ArrowRight),
                action: Action::ApplyMomentum {
                    target: Target::name("player"),
                    value: (5.0, 0.0),
                },
                target: Target::name("player"),
            },
            Target::name("player"),
        )
        .on_enter(|canvas| {
            // Runs when this scene is loaded
            let mut cam = Camera::new((8000.0, 2000.0), canvas.get_virtual_size());
            cam.follow(Some(Target::name("player")));
            canvas.set_camera(cam);
        })
        .on_exit(|canvas| {
            // Runs just before the scene is unloaded
            canvas.clear_camera();
        })
}
```

### Registering and loading scenes

Register all scenes at startup, then load the first one:

```rust
fn new(ctx: &mut Context) -> impl Drawable {
    let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);

    canvas.add_scene(build_menu(ctx));
    canvas.add_scene(build_level_1(ctx));
    canvas.add_scene(build_level_2(ctx));
    canvas.add_scene(build_game_over(ctx));

    canvas.load_scene("menu");
    canvas
}
```

### Transitioning between scenes

Call `canvas.load_scene(name)` from anywhere. The transition happens in this order:

1. The current scene's `on_exit` callback fires
2. All of the current scene's objects are removed from the canvas
3. The new scene's objects are added
4. The new scene's `on_enter` callback fires

```rust
canvas.on_tick(|canvas| {
    if let Some(player) = canvas.get_game_object("player") {
        if player.position.1 > 3500.0 {
            canvas.load_scene("game_over");
        }
    }
});
```

### Checking the active scene

```rust
if canvas.is_scene("level_1") {
    // only runs during level 1
}

match canvas.active_scene() {
    Some("level_1") => { /* ... */ }
    Some("menu")    => { /* ... */ }
    _               => {}
}
```

---

## 11. Camera

The camera offsets what the player sees without changing any object's actual world position. Use it for any game world larger than the viewport.

### Creating and attaching a camera

```rust
use quartz::Camera;

let mut cam = Camera::new(
    (16_000.0, 3_000.0),       // world size — match your level dimensions
    canvas.get_virtual_size(), // viewport = the virtual resolution
);

cam.follow(Some(Target::name("player")));
cam.lerp_speed = 0.10;   // 0.0 = frozen, 1.0 = instant snap

canvas.set_camera(cam);
```

The camera is almost always set inside `on_enter` and cleared in `on_exit`:

```rust
.on_enter(|canvas| {
    let mut cam = Camera::new((WORLD_W, WORLD_H), canvas.get_virtual_size());
    cam.follow(Some(Target::name("player")));
    cam.lerp_speed = 0.08;
    canvas.set_camera(cam);
})
.on_exit(|canvas| {
    canvas.clear_camera();
})
```

### Camera API reference

| Method / Field | Description |
|---|---|
| `Camera::new(world_size, viewport_size)` | Create a camera |
| `cam.follow(Some(target))` | Smoothly follow a target each tick |
| `cam.follow(None)` | Stop following — camera stays put |
| `cam.center_on(wx, wy)` | Snap camera to center on a world point immediately |
| `cam.lerp_speed` | Follow smoothness (range 0.0–1.0). `0.10` is a good default |
| `cam.position` | Current `(x, y)` offset — top-left of the visible area in world space |
| `cam.world_size` | Total world dimensions (used for edge clamping) |

### Tuning lerp speed

```rust
cam.lerp_speed = 0.05;   // very floaty, slow to catch up
cam.lerp_speed = 0.12;   // standard platformer feel
cam.lerp_speed = 0.25;   // snappy
cam.lerp_speed = 1.0;    // instant, no lerp
```

### Manually moving the camera

```rust
if let Some(cam) = canvas.camera_mut() {
    cam.center_on(5000.0, 1500.0);  // snap to a world location
}
```

---

## 12. Animated Sprites

Quartz plays GIF files as frame animations. Each GIF frame becomes one animation frame, cycled automatically by the engine every tick.

### Embedding a GIF

Use `include_bytes!` to embed GIF data directly into the binary:

```rust
static RUN_GIF:  &[u8] = include_bytes!("../assets/run.gif");
static IDLE_GIF: &[u8] = include_bytes!("../assets/idle.gif");
static JUMP_GIF: &[u8] = include_bytes!("../assets/jump.gif");
```

### Creating an AnimatedSprite

```rust
use quartz::AnimatedSprite;

let sprite = AnimatedSprite::new(
    RUN_GIF,          // &[u8] — the GIF data
    (80.0, 120.0),    // display size in virtual pixels (width, height)
    12.0,             // playback speed in frames per second
).expect("Failed to load animation");
```

### Attaching to a GameObject at creation

```rust
let player = GameObject::new_rect(ctx, "player".to_string(), None,
    (80.0, 120.0), (300.0, 900.0), vec![], (0.0, 0.0), (0.85, 1.0), 0.55)
    .with_animation(sprite);
```

Passing `None` as the image is fine when using an animated sprite — the sprite provides the image each tick.

### Swapping animations at runtime via Action::SetAnimation

```rust
// Switch to run animation when the player starts moving right
canvas.add_event(
    GameEvent::KeyPress {
        key: Key::Named(NamedKey::ArrowRight),
        action: Action::SetAnimation {
            target: Target::name("player"),
            animation_bytes: RUN_GIF,
            fps: 12.0,
        },
        target: Target::name("player"),
    },
    Target::name("player"),
);

// Switch back to idle when the key is released
canvas.add_event(
    GameEvent::KeyRelease {
        key: Key::Named(NamedKey::ArrowRight),
        action: Action::SetAnimation {
            target: Target::name("player"),
            animation_bytes: IDLE_GIF,
            fps: 8.0,
        },
        target: Target::name("player"),
    },
    Target::name("player"),
);
```

### Swapping animations from a tick callback

For finer control (e.g., switching based on multiple keys or state):

```rust
canvas.on_tick(|canvas| {
    let moving = canvas.is_key_held(&Key::Named(NamedKey::ArrowRight))
              || canvas.is_key_held(&Key::Named(NamedKey::ArrowLeft));

    let anim = if moving { RUN_GIF } else { IDLE_GIF };
    let fps  = if moving { 12.0 }   else { 6.0 };

    canvas.run(Action::SetAnimation {
        target: Target::name("player"),
        animation_bytes: anim,
        fps,
    });
});
```

### AnimatedSprite methods (direct access)

```rust
if let Some(player) = canvas.get_game_object_mut("player") {
    if let Some(sprite) = &mut player.animated_sprite {
        sprite.reset();                // jump back to frame 0
        sprite.set_frame(3);           // jump to a specific frame index
        sprite.set_fps(24.0);          // change playback speed
        let n = sprite.frame_count();  // total frames in the GIF
    }
}
```

---

## 13. Physics Deep Dive

Understanding the physics tick order lets you precisely tune how your game feels.

### Per-tick physics sequence

For each visible object, every tick, in this exact order:

```
1.  momentum.y  += gravity
2.  position    += momentum
3.  momentum    *= resistance   (per axis)
4.  if |momentum.x| < 0.001  →  momentum.x = 0.0
    if |momentum.y| < 0.001  →  momentum.y = 0.0
```

### Platform collision behavior

When a non-platform object has **downward momentum** (`momentum.y > 0`) and overlaps a platform object:
- The object's `position.y` is snapped to the platform's top surface
- `momentum.y` is set to `0.0`

This only triggers when moving downward, so jumping up through a platform from below works naturally.

### Object–object collision behavior

When two non-platform objects overlap, both fire their `GameEvent::Collision` events. No automatic position correction occurs — you decide what happens (destroy, bounce, score a point, etc.).

### Physics feel guide

| Goal | Setting |
|---|---|
| Floaty jump | Lower gravity (`0.3`), higher jump impulse (`-28.0`) |
| Snappy jump | Higher gravity (`0.8`), lower jump impulse (`-18.0`) |
| Icy / slippery floor | High x-resistance (`0.98`) — momentum fades slowly |
| Responsive, grippy feel | Low x-resistance (`0.7`) — momentum fades fast |
| No vertical air drag | y-resistance = `1.0` (default — let gravity do the work) |
| Floating / antigravity | Set `gravity = 0.0` or even negative (`-0.1`) |
| Heavy boulder | High gravity (`1.2`), high resistance (`0.95`) |

### Typical platformer values

```rust
gravity         = 0.55
jump impulse    = (0.0, -22.0)  // applied once on KeyPress
resistance      = (0.82, 1.0)
move per tick   = (5.0, 0.0)    // applied on KeyHold
```

### Making a truly static object

Zero gravity, zero momentum, resistance = `(1.0, 1.0)`:

```rust
GameObject::new_rect(ctx, "wall".to_string(), Some(img),
    (30.0, 400.0), (500.0, 700.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0)
    .as_platform()
```

---

## 14. Custom Events

Custom events let you define named game-logic hooks that fire whenever `Action::Custom` is run with the matching name.

### Registering a handler

```rust
canvas.register_custom_event("player_died".to_string(), |canvas| {
    canvas.play_sound("assets/death.wav");
    canvas.load_scene("game_over");
});
```

### Triggering from an event

```rust
// When the player touches a spike, fire "player_died"
canvas.add_event(
    GameEvent::Collision {
        action: Action::Custom { name: "player_died".to_string() },
        target: Target::name("player"),
    },
    Target::name("player"),
);
```

### Triggering directly

```rust
// From anywhere in a tick callback or key handler
canvas.run(Action::Custom { name: "player_died".to_string() });
```

### Using GameEvent::Custom as an indirect trigger

An object can carry a `GameEvent::Custom` in its event list. When the engine processes custom events each tick, it calls the matching handler:

```rust
canvas.add_event(
    GameEvent::Custom {
        name: "spawn_wave".to_string(),
        target: Target::name("wave_trigger"),
    },
    Target::name("wave_trigger"),
);

canvas.register_custom_event("spawn_wave".to_string(), |canvas| {
    // spawn a new wave of enemies
});
```

---

## 15. Infinite Scrolling

Quartz has built-in support for seamlessly looping backgrounds. Tag two or more background objects with `"scroll"`, give them a leftward momentum, and the engine automatically recycles any panel that scrolls off the left edge by moving it to the right of the rightmost panel.

### Setup

```rust
// Two background panels placed side by side, each as wide as the viewport
let bg1 = make_bg(ctx, "bg1", (3840.0, 2160.0), (0.0, 0.0))
    .with_tag("scroll")
    .with_momentum((-2.0, 0.0));

let bg2 = make_bg(ctx, "bg2", (3840.0, 2160.0), (3840.0, 0.0))
    .with_tag("scroll")
    .with_momentum((-2.0, 0.0));

canvas.add_game_object("bg1".to_string(), bg1);
canvas.add_game_object("bg2".to_string(), bg2);
```

You need **at least 2** objects tagged `"scroll"` for the system to activate. The engine handles the recycling each tick with no additional code needed.

---

## 16. Sound

Sound plays asynchronously on a background thread, so it never stalls the game loop.

```rust
canvas.play_sound("assets/jump.wav");
canvas.play_sound("assets/coin.wav");
canvas.play_sound("assets/music.mp3");
```

Call this from anywhere — tick callbacks, event handlers, or custom event handlers. Supported formats depend on the `rodio` backend (WAV, MP3, OGG, FLAC are all commonly supported).

---

## 17. Complete Example — Grass World Platformer

This is the full annotated version of the sample project bundled with the engine. It demonstrates solid-color objects, platforms, rocks, a player with gravity and friction, arrow-key controls, and a lerp-following camera over a 16,000px-wide world.

```rust
use quartz::{Key, Context, Image, ShapeType, Canvas, GameObject, Action, Target,
             GameEvent, CanvasMode, Scene, Camera, Anchor, Location};
use ramp::prism;
use prism::drawable::Drawable;
use prism::event::NamedKey;

pub struct GrassWorld;

const WORLD_W: f32 = 16_000.0;
const WORLD_H: f32 = 3_000.0;
const GROUND_Y: f32 = WORLD_H - 400.0;  // y-coordinate of the top of the grass strip

impl GrassWorld {
    pub fn new(ctx: &mut Context) -> impl Drawable {
        let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
        canvas.add_scene(Self::grass_scene(ctx));
        canvas.load_scene("grass");
        canvas
    }

    fn grass_scene(ctx: &mut Context) -> Scene {
        // ── Sky background ───────────────────────────────────────────────────
        let sky = Self::rect(ctx, "sky", (WORLD_W, WORLD_H), (0.0, 0.0),
                             [135, 206, 235, 255], 0.0);

        // ── Ground layers ────────────────────────────────────────────────────
        let grass = Self::rect(ctx, "grass_ground", (WORLD_W, 80.0),
                               (0.0, GROUND_Y), [60, 179, 60, 255], 0.0)
                               .as_platform();  // player lands on this

        let dirt = Self::rect(ctx, "dirt", (WORLD_W, 400.0),
                              (0.0, GROUND_Y + 80.0), [139, 90, 43, 255], 0.0);

        // ── Decorative rocks sitting on the grass surface ─────────────────────
        let rocks: &[(&str, f32, f32, f32)] = &[
            ("rock_0",    620.0,  55.0, 44.0),
            ("rock_1",   1150.0,  40.0, 32.0),
            ("rock_2",   1820.0,  65.0, 52.0),
            // ... more rocks
        ];

        // ── Elevated platforms scattered across the level ─────────────────────
        let platforms: &[(&str, f32, f32, f32)] = &[
            ("plat_0",   500.0, GROUND_Y - 320.0, 400.0),
            ("plat_1",  1100.0, GROUND_Y - 520.0, 350.0),
            ("plat_2",  1700.0, GROUND_Y - 360.0, 300.0),
            // ... more platforms
        ];

        // ── Player — starts near the left edge, standing on the grass ─────────
        let player = Self::rect(ctx, "player", (70.0, 110.0),
                                (300.0, GROUND_Y - 110.0), [220, 100, 80, 255], 0.55);

        // ── Assemble the scene ───────────────────────────────────────────────
        let mut scene = Scene::new("grass")
            .with_object("sky",          sky)
            .with_object("dirt",         dirt)
            .with_object("grass_ground", grass);

        // Add rocks (decorative, no collision)
        for (id, x, w, h) in rocks {
            let rock = Self::rect(ctx, id, (*w, *h),
                (*x, GROUND_Y - h), [120, 110, 100, 255], 0.0);
            scene = scene.with_object(*id, rock);
        }

        // Add platforms (player can land on these)
        for (id, x, y, w) in platforms {
            let plat = Self::rect(ctx, id, (*w, 30.0),
                (*x, *y), [160, 120, 70, 255], 0.0).as_platform();
            scene = scene.with_object(*id, plat);
        }

        scene
            .with_object("player", player)

            // ── Arrow key movement ───────────────────────────────────────────
            .with_event(
                GameEvent::KeyHold {
                    key: Key::Named(NamedKey::ArrowRight),
                    action: Action::ApplyMomentum {
                        target: Target::ByName("player".to_string()),
                        value: (5.0, 0.0),
                    },
                    target: Target::ByName("player".to_string()),
                },
                Target::ByName("player".to_string()),
            )
            .with_event(
                GameEvent::KeyHold {
                    key: Key::Named(NamedKey::ArrowLeft),
                    action: Action::ApplyMomentum {
                        target: Target::ByName("player".to_string()),
                        value: (-5.0, 0.0),
                    },
                    target: Target::ByName("player".to_string()),
                },
                Target::ByName("player".to_string()),
            )

            // ── Jump ─────────────────────────────────────────────────────────
            .with_event(
                GameEvent::KeyPress {
                    key: Key::Named(NamedKey::ArrowUp),
                    action: Action::ApplyMomentum {
                        target: Target::ByName("player".to_string()),
                        value: (0.0, -22.0),  // negative y = upward
                    },
                    target: Target::ByName("player".to_string()),
                },
                Target::ByName("player".to_string()),
            )

            // ── Camera: follow the player, clean up when leaving ──────────────
            .on_enter(|canvas| {
                let mut cam = Camera::new(
                    (WORLD_W, WORLD_H),
                    canvas.get_virtual_size(),
                );
                cam.follow(Some(Target::ByName("player".to_string())));
                cam.lerp_speed = 0.10;
                canvas.set_camera(cam);
            })
            .on_exit(|canvas| {
                canvas.clear_camera();
            })
    }

    // ── Utility: build a solid-color rectangle ───────────────────────────────
    fn rect(
        ctx: &mut Context,
        id: &str,
        size: (f32, f32),
        position: (f32, f32),
        rgba: [u8; 4],
        gravity: f32,
    ) -> GameObject {
        let image = Image {
            shape: ShapeType::Rectangle(0.0, size, 0.0),
            image: image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)).into(),
            color: None,
        };
        let mut obj = GameObject::new_rect(
            ctx,
            id.to_string(),
            Some(image),
            size,
            position,
            vec![],
            (0.0, 0.0),
            (0.82, 1.0),   // horizontal friction, no vertical damping
            gravity,
        );
        obj.update_image_shape();
        obj
    }
}

ramp::run! { |ctx: &mut Context, _assets: Assets| {
    GrassWorld::new(ctx)
}}
```

---

## 18. Common Game Recipes

### Recipe 1: Standard platformer controls

```rust
// Move left/right with acceleration
canvas.add_event(GameEvent::KeyHold {
    key: Key::Named(NamedKey::ArrowRight),
    action: Action::ApplyMomentum { target: Target::name("player"), value: (4.0, 0.0) },
    target: Target::name("player"),
}, Target::name("player"));

canvas.add_event(GameEvent::KeyHold {
    key: Key::Named(NamedKey::ArrowLeft),
    action: Action::ApplyMomentum { target: Target::name("player"), value: (-4.0, 0.0) },
    target: Target::name("player"),
}, Target::name("player"));

// Jump (Space bar)
canvas.add_event(GameEvent::KeyPress {
    key: Key::Named(NamedKey::Space),
    action: Action::ApplyMomentum { target: Target::name("player"), value: (0.0, -22.0) },
    target: Target::name("player"),
}, Target::name("player"));
```

### Recipe 2: Collectible coins

```rust
// Spawn coins with a shared tag
for (i, (x, y)) in coin_positions.iter().enumerate() {
    let coin = make_coin(ctx, *x, *y).with_tag("coin");
    canvas.add_game_object(format!("coin_{i}"), coin);
}

// Detect collection every tick
canvas.on_tick(|canvas| {
    // Collect all coin names that are currently colliding with the player
    let collected: Vec<String> = (0..50)
        .map(|i| format!("coin_{i}"))
        .filter(|name| {
            canvas.get_game_object(name).map_or(false, |c| c.visible)
                && canvas.collision_between(
                    &Target::name("player"),
                    &Target::name(name),
                )
        })
        .collect();

    for name in collected {
        canvas.remove_game_object(&name);
        canvas.play_sound("assets/coin.wav");
    }
});
```

### Recipe 3: Enemy that bounces between walls

```rust
// Enemy with leftward momentum and a bounce handler
let enemy = GameObject::new_rect(ctx, "enemy_1".to_string(), Some(img),
    (80.0, 120.0), (600.0, GROUND_Y - 120.0),
    vec![], (-2.0, 0.0), (1.0, 1.0), 0.55);

canvas.add_game_object("enemy_1".to_string(), enemy);

// Flip direction on boundary hit
canvas.add_event(
    GameEvent::BoundaryCollision {
        action: Action::Custom { name: "bounce_enemy_1".to_string() },
        target: Target::name("enemy_1"),
    },
    Target::name("enemy_1"),
);

canvas.register_custom_event("bounce_enemy_1".to_string(), |canvas| {
    if let Some(enemy) = canvas.get_game_object_mut("enemy_1") {
        enemy.momentum.0 *= -1.0;  // reverse horizontal direction
    }
});
```

### Recipe 4: Shooting projectiles

```rust
static BULLET_GIF: &[u8] = include_bytes!("../assets/bullet.gif");

fn make_bullet(ctx: &mut Context) -> GameObject {
    let sprite = AnimatedSprite::new(BULLET_GIF, (20.0, 10.0), 8.0).unwrap();
    GameObject::new_rect(ctx, "bullet".to_string(), None,
        (20.0, 10.0), (0.0, 0.0),
        vec!["bullet".to_string()],
        (18.0, 0.0),   // flies right at 18px/tick
        (1.0, 1.0),    // no drag
        0.0,           // no gravity
    ).with_animation(sprite)
}

// Spawn on Space press
canvas.add_event(
    GameEvent::KeyPress {
        key: Key::Named(NamedKey::Space),
        action: Action::Spawn {
            object: Box::new(make_bullet(ctx)),
            location: Location::OnTarget {
                target: Box::new(Target::name("player")),
                anchor: Anchor { x: 1.0, y: 0.5 },
                offset: (5.0, 0.0),
            },
        },
        target: Target::name("player"),
    },
    Target::name("player"),
);

// Remove bullets when they leave the screen
canvas.add_event(
    GameEvent::BoundaryCollision {
        action: Action::Remove { target: Target::tag("bullet") },
        target: Target::tag("bullet"),
    },
    Target::tag("bullet"),
);
```

### Recipe 5: Multi-scene game with a main menu

```rust
fn build_menu(ctx: &mut Context) -> Scene {
    let title = /* make a title text object */;
    let hint  = /* "Press Enter to start" object */;

    Scene::new("menu")
        .with_object("title", title)
        .with_object("hint",  hint)
        .with_event(
            GameEvent::KeyPress {
                key: Key::Named(NamedKey::Enter),
                action: Action::Custom { name: "start_game".to_string() },
                target: Target::name("hint"),
            },
            Target::name("hint"),
        )
        .on_enter(|canvas| {
            canvas.register_custom_event("start_game".to_string(), |canvas| {
                canvas.load_scene("level_1");
            });
        })
}
```

### Recipe 6: Death and respawn

```rust
canvas.on_tick(|canvas| {
    let fell = canvas.get_game_object("player")
        .map_or(false, |p| p.position.1 > WORLD_H + 200.0);

    if fell {
        // Reset position and velocity
        canvas.run(Action::Teleport {
            target: Target::name("player"),
            location: Location::at(300.0, GROUND_Y - 110.0),
        });
        canvas.run(Action::SetMomentum {
            target: Target::name("player"),
            value: (0.0, 0.0),
        });
        canvas.play_sound("assets/respawn.wav");
    }
});
```

### Recipe 7: Objects that follow the player

```rust
canvas.on_tick(|canvas| {
    let player_pos = canvas.get_game_object("player")
        .map(|p| p.position)
        .unwrap_or((0.0, 0.0));

    if let Some(follower) = canvas.get_game_object_mut("follower") {
        let dx = player_pos.0 - follower.position.0;
        let dy = player_pos.1 - follower.position.1;
        // Move 5% of the distance toward the player each tick
        follower.momentum.0 = dx * 0.05;
        follower.momentum.1 = dy * 0.05;
    }
});
```

---

## 19. Quick Reference Card

### Coordinate system

```
(0, 0) ─────────────→ +X
  │
  │     World space
  ↓
 +Y
```

- Origin `(0, 0)` is the **top-left** corner of the world
- X increases to the **right**
- Y increases **downward**
- Negative `momentum.y` → moves **up** (jump impulse is negative)
- Positive `momentum.y` → moves **down** (gravity is positive)

### Physics cheat sheet

| Value | Effect |
|---|---|
| `gravity = 0.0` | Static / floating object |
| `gravity = 0.3` | Gentle, floaty gravity |
| `gravity = 0.55` | Standard platformer feel |
| `gravity = 1.0` | Heavy, fast fall |
| `resistance.x = 1.0` | No horizontal friction — slides forever |
| `resistance.x = 0.85` | Standard platformer friction |
| `resistance.x = 0.6` | Very sticky — stops quickly |
| `momentum.y = -15.0` | Short hop |
| `momentum.y = -22.0` | Standard jump |
| `momentum.y = -30.0` | High / floaty jump |

### Anchor point cheat sheet

```rust
Anchor { x: 0.0, y: 0.0 }  // top-left
Anchor { x: 0.5, y: 0.0 }  // top-center
Anchor { x: 1.0, y: 0.0 }  // top-right
Anchor { x: 0.5, y: 0.5 }  // center
Anchor { x: 0.0, y: 1.0 }  // bottom-left
Anchor { x: 0.5, y: 1.0 }  // bottom-center
Anchor { x: 1.0, y: 1.0 }  // bottom-right
Anchor { x: 1.0, y: 0.5 }  // right-center
Anchor { x: 0.0, y: 0.5 }  // left-center
```

### Solid-color image helper

A pattern you'll use constantly:

```rust
fn solid_color_image(width: f32, height: f32, rgba: [u8; 4]) -> Image {
    Image {
        shape: ShapeType::Rectangle(0.0, (width, height), 0.0),
        image: image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)).into(),
        color: None,
    }
}
```

### Scene lifecycle summary

```
canvas.load_scene("name")
         │
         ├─ 1. current scene on_exit() fires
         ├─ 2. current scene's objects removed
         ├─ 3. new scene's objects added
         └─ 4. new scene on_enter() fires
```

### Event dispatch order per tick

```
1. on_tick callbacks (your closures)
2. KeyHold events (for all held keys)
3. Tick events (on all objects)
4. Custom events
5. Physics update (gravity, position, resistance)
6. Collision detection & resolution
```

---

*End of Quartz Game Engine Tutorial*