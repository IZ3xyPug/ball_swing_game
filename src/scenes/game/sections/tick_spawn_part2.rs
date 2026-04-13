            // ── Spawn score x2 pickups ahead of the player ─────────────────
            let mut score_x2_spawned_this_tick = 0usize;
            while score_x2_spawned_this_tick < 1
                && s.score_x2_rightmost < s.px + GEN_AHEAD
                && !s.score_x2_free.is_empty()
            {
                let gap = lcg_range(&mut s.seed, SCORE_X2_GAP_MIN, SCORE_X2_GAP_MAX);
                let x2_x = s.score_x2_rightmost + gap;
                let raw_x2_y = lcg_range(&mut s.seed, VH * 0.26, VH * 0.64);
                let x2_y = if s.gravity_dir < 0.0 {
                    VH - raw_x2_y - SCORE_X2_H
                } else {
                    raw_x2_y
                };
                s.score_x2_rightmost = x2_x;
                let Some(id) = s.score_x2_free.pop() else { break; };
                s.score_x2_live.push(id.clone());
                score_x2_spawned_this_tick += 1;
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&id) {
                    obj.position = (x2_x, x2_y);
                    obj.visible = true;
                }

                s = st.lock().unwrap();
            }

            // ── Spawn flappy-style gate obstacles ahead of the player ───────
            // Gap obstacles disabled: procedural gate clusters removed from gameplay loop.
            let mut gates_spawned_this_tick = 0usize;
            while gates_spawned_this_tick < GATES_SPAWN_BUDGET_PER_TICK
                && GATES_ENABLED
                && s.gate_rightmost < s.px + GEN_AHEAD
                && !s.gate_free.is_empty()
            {
                let gap = lcg_range(&mut s.seed, GATE_GAP_MIN, GATE_GAP_MAX);
                let base_x = s.gate_rightmost + gap.max(GATE_MIN_CLUSTER_SEPARATION);
                let gaps_in_cluster = 2 + ((lcg(&mut s.seed) * 3.0) as usize);
                let cluster_spacing = GATE_MIN_CLUSTER_SEPARATION;
                let mut spawn_batch: Vec<(String, String, f32, Option<(String, f32, f32)>)> = Vec::new();

                for i in 0..gaps_in_cluster {
                    let Some(gid) = s.gate_free.pop() else { break; };
                    let gate_x = base_x + i as f32 * cluster_spacing;
                    s.gate_live.push(gid.clone());
                    let top_id = format!("{gid}_top");
                    let bot_id = format!("{gid}_bot");

                    // Spawn a hook near each gate gap when possible.
                    let hook_spawn = if let Some(hook_id) = s.pool_free.pop() {
                        let mut hx = gate_x - 450.0;
                        let hook_spinner_min_x_gap = HOOK_SPINNER_MIN_X_GAP;
                        for spinner_name in &s.spinner_live {
                            if let Some(spinner_obj) = c.get_game_object(spinner_name) {
                                let spinner_center_x = spinner_obj.position.0 + SPINNER_W * 0.5;
                                let dx = hx - spinner_center_x;
                                if dx.abs() < hook_spinner_min_x_gap {
                                    let dir = if dx >= 0.0 { 1.0 } else { -1.0 };
                                    hx += dir * HOOK_SPINNER_PUSH_X;
                                }
                            }
                        }
                        let hy = 650.0;
                        s.live_hooks.push(hook_id.clone());
                        Some((hook_id, hx, hy))
                    } else {
                        None
                    };

                    spawn_batch.push((top_id, bot_id, gate_x, hook_spawn));
                }

                if spawn_batch.is_empty() {
                    break;
                }

                let last_gate_x = spawn_batch.last().map(|(_, _, x, _)| *x).unwrap_or(base_x);
                s.gate_rightmost = last_gate_x;
                gates_spawned_this_tick += 1;
                let spinner_ids = s.spinner_live.clone();
                drop(s);

                for (top_id, bot_id, gate_x, hook_spawn) in &spawn_batch {
                    if let Some(obj) = c.get_game_object_mut(top_id) {
                        obj.position = (*gate_x, -GATE_VERTICAL_OVERFLOW);
                        obj.size = (GATE_W, GATE_TOP_SEG_H);
                        obj.visible = true;
                    }
                    if let Some(obj) = c.get_game_object_mut(bot_id) {
                        obj.position = (*gate_x, GATE_TOP_BASE_H + GATE_GAP_H);
                        obj.size = (GATE_W, GATE_BOT_SEG_H);
                        obj.visible = true;
                    }

                    // Keep gate opening clear: push overlapping spinners away.
                    for sid in &spinner_ids {
                        if let Some(sp) = c.get_game_object_mut(sid) {
                            let overlaps = sp.position.0 + SPINNER_W > *gate_x - 80.0
                                && sp.position.0 < *gate_x + GATE_W + 80.0;
                            if overlaps {
                                sp.position.0 = *gate_x + GATE_W + 240.0;
                            }
                        }
                    }

                    if let Some((hook_id, hx, hy)) = hook_spawn {
                        if let Some(obj) = c.get_game_object_mut(hook_id) {
                            obj.position = (*hx - HOOK_R, *hy - HOOK_R);
                            obj.visible = true;
                            obj.set_image(Image {
                                shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                                image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                                color: None,
                            });
                        }
                    }
                }

                s = st.lock().unwrap();
            }

            // ── Spawn sparse coins ahead of the player ───────────────────────
            let mut coin_batches_spawned_this_tick = 0usize;
            while coin_batches_spawned_this_tick < COIN_BATCHES_BUDGET_PER_TICK
                && s.coin_rightmost < s.px + GEN_AHEAD
                && !s.coin_free.is_empty()
            {
                let gap = lcg_range(&mut s.seed, COIN_GAP_MIN, COIN_GAP_MAX);
                let desired_start_x = s.coin_rightmost + gap;
                let spawn_array = s.coin_free.len() >= COIN_ARRAY_COUNT && lcg(&mut s.seed) < COIN_ARRAY_CHANCE;
                let mut spawn_batch: Vec<(String, f32, f32, usize)> = Vec::new();
                let mut spawned_start_x = desired_start_x;
                let coin_anim_frames = coin_spawn_anim
                    .as_ref()
                    .map(|a| a.frame_count().max(1))
                    .unwrap_or(1);
                // Keep each array internally synced, but desync arrays/singles from each other.
                let array_phase_frame = (lcg(&mut s.seed) * coin_anim_frames as f32) as usize;

                if spawn_array {
                    // Anchor arc arrays to a nearby live hook so placement is
                    // relative to grab nodes, not an absolute world Y range.
                    let mut best_anchor: Option<(f32, f32)> = None; // (hook_center_x, hook_raw_y)
                    let mut best_score = f32::INFINITY;
                    let hook_ids = s.live_hooks.clone();
                    for hid in &hook_ids {
                        if let Some(hook_obj) = c.get_game_object(hid) {
                            let hook_center_x = hook_obj.position.0 + HOOK_R;
                            let hook_center_world_y = hook_obj.position.1 + HOOK_R;
                            let hook_raw_y = if s.gravity_dir < 0.0 {
                                VH - hook_center_world_y
                            } else {
                                hook_center_world_y
                            };
                            let candidate_start_x = hook_center_x + COIN_ARRAY_HOOK_DX;
                            let score = (candidate_start_x - desired_start_x).abs();
                            if score < best_score {
                                best_score = score;
                                best_anchor = Some((hook_center_x, hook_raw_y));
                            }
                        }
                    }

                    let center_raw_y = if let Some((hook_center_x, hook_raw_y)) = best_anchor {
                        spawned_start_x = hook_center_x + COIN_ARRAY_HOOK_DX;
                        hook_raw_y + COIN_ARRAY_HOOK_DY
                    } else {
                        // Fallback when no hook object is available yet.
                        let center_min = (COIN_ARRAY_Y_MIN + COIN_CURVE_RISE).min(COIN_ARRAY_Y_MAX);
                        lcg_range(&mut s.seed, center_min, COIN_ARRAY_Y_MAX)
                    };

                    let half = (COIN_ARRAY_COUNT as f32 - 1.0) * 0.5;

                    // Curved 5-coin formation, left-to-right from the hook offset anchor.
                    for i in 0..COIN_ARRAY_COUNT {
                        let x = spawned_start_x + i as f32 * COIN_ARRAY_SPACING;
                        let t = i as f32 - half;
                        let norm = if half > 0.0 { (t.abs() / half).clamp(0.0, 1.0) } else { 0.0 };
                        let arch = 1.0 - norm * norm;
                        let raw_y = center_raw_y - arch * COIN_CURVE_RISE;
                        let y = if s.gravity_dir < 0.0 { VH - raw_y } else { raw_y };

                        let Some(id) = s.coin_free.pop() else { break; };
                        s.coin_live.push(id.clone());
                        spawn_batch.push((id, x, y, array_phase_frame.min(coin_anim_frames - 1)));
                    }
                } else {
                    // Single coins can spawn anywhere below the array band.
                    let raw_y = lcg_range(&mut s.seed, COIN_SINGLE_Y_MIN, COIN_SINGLE_Y_MAX);
                    let y = if s.gravity_dir < 0.0 { VH - raw_y } else { raw_y };
                    if let Some(id) = s.coin_free.pop() {
                        s.coin_live.push(id.clone());
                        let single_phase = ((lcg(&mut s.seed) * coin_anim_frames as f32) as usize)
                            .min(coin_anim_frames - 1);
                        spawn_batch.push((id, desired_start_x, y, single_phase));
                    }
                }

                if spawn_batch.is_empty() {
                    break;
                }

                s.coin_rightmost = if spawn_array {
                    spawned_start_x + (COIN_ARRAY_COUNT as f32 - 1.0) * COIN_ARRAY_SPACING
                } else {
                    desired_start_x
                };
                coin_batches_spawned_this_tick += 1;
                drop(s);

                for (id, coin_x, coin_y, phase_frame) in &spawn_batch {
                    if let Some(obj) = c.get_game_object_mut(id) {
                        obj.position = (*coin_x - COIN_R, *coin_y - COIN_R);
                        obj.visible = true;
                        obj.set_image(coin_spawn_image.clone());
                        if let Some(anim) = &coin_spawn_anim {
                            if obj.animated_sprite.is_none() {
                                obj.set_animation(anim.clone());
                            }
                            if let Some(a) = obj.animated_sprite.as_mut() {
                                a.set_frame(*phase_frame);
                            }
                        }
                    }
                }

                s = st.lock().unwrap();
            }

            // ── Cull pads behind the player ───────────────────────────────────
            let pad_cutoff = s.px - VW * 1.5;
            let pads_remove: Vec<String> = s.pad_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + PAD_W < pad_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &pads_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3000.0, -3000.0);
                }
            }
            let pads_rm_set: HashSet<&str> = pads_remove.iter().map(|n| n.as_str()).collect();
            s.pad_live.retain(|n| !pads_rm_set.contains(n.as_str()));
            for name in &pads_remove {
                s.pad_origins.retain(|(n, _, _, _, _)| n != name);
            }
            for name in pads_remove {
                s.pad_free.push(name);
            }

            // ── Cull spinning obstacles behind the player ────────────────────
            let spin_cutoff = s.px - VW * 1.5;
            let spins_remove: Vec<String> = s.spinner_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + SPINNER_W < spin_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &spins_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3500.0, -3500.0);
                    obj.rotation_momentum = 0.0;
                }
            }
            let spins_rm_set: HashSet<&str> = spins_remove.iter().map(|n| n.as_str()).collect();
            s.spinner_live.retain(|n| !spins_rm_set.contains(n.as_str()));
            s.spinner_origins.retain(|(id, _, _, _, _)| !spins_rm_set.contains(id.as_str()));
            for name in spins_remove {
                s.spinner_free.push(name);
            }

            // ── Cull coins behind the player ──────────────────────────────────
            let coin_cutoff = s.px - VW * 1.5;
            let coins_remove: Vec<String> = s.coin_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + COIN_R * 2.0 < coin_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &coins_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3700.0, -3700.0);
                }
            }
            let coins_rm_set: HashSet<&str> = coins_remove.iter().map(|n| n.as_str()).collect();
            s.coin_live.retain(|n| !coins_rm_set.contains(n.as_str()));
            s.coin_magnet_locked.retain(|n| !coins_rm_set.contains(n.as_str()));
            for name in coins_remove {
                s.coin_free.push(name);
            }

            // ── Cull gravity flips behind the player ─────────────────────────
            let flip_cutoff = s.px - VW * 1.5;
            let flips_remove: Vec<String> = s.flip_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + FLIP_W < flip_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &flips_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3800.0, -3800.0);
                }
            }
            let flips_rm_set: HashSet<&str> = flips_remove.iter().map(|n| n.as_str()).collect();
            s.flip_live.retain(|n| !flips_rm_set.contains(n.as_str()));
            for name in flips_remove {
                s.flip_free.push(name);
            }

            // ── Cull score x2 pickups behind the player ─────────────────────
            let score_x2_cutoff = s.px - VW * 1.5;
            let score_x2_remove: Vec<String> = s.score_x2_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + SCORE_X2_W < score_x2_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &score_x2_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3850.0, -3850.0);
                }
            }
            let score_x2_rm_set: HashSet<&str> = score_x2_remove.iter().map(|n| n.as_str()).collect();
            s.score_x2_live.retain(|n| !score_x2_rm_set.contains(n.as_str()));
            for name in score_x2_remove {
                s.score_x2_free.push(name);
            }

            // ── Cull gates behind the player ─────────────────────────────────
            let gate_cutoff = s.px - VW * 1.5;
            let gates_remove: Vec<String> = s.gate_live.iter()
                .filter(|name| {
                    let top_id = format!("{name}_top");
                    c.get_game_object(&top_id)
                        .map(|o| o.position.0 + GATE_W < gate_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &gates_remove {
                let top_id = format!("{name}_top");
                let bot_id = format!("{name}_bot");
                if let Some(obj) = c.get_game_object_mut(&top_id) {
                    obj.visible = false;
                    obj.position = (-3900.0, -3900.0);
                }
                if let Some(obj) = c.get_game_object_mut(&bot_id) {
                    obj.visible = false;
                    obj.position = (-3900.0, -3900.0);
                }
            }
            let gates_rm_set: HashSet<&str> = gates_remove.iter().map(|n| n.as_str()).collect();
            s.gate_live.retain(|n| !gates_rm_set.contains(n.as_str()));
            for name in gates_remove {
                s.gate_free.push(name);
            }

            // ── Animate moving pads ───────────────────────────────────────────
            let ticks = s.ticks;
            let pad_origins_snap: Vec<(String, f32, f32, f32, f32)> = s.pad_origins.clone();
            drop(s);
            for (id, origin_x, amp, speed, phase) in &pad_origins_snap {
                let offset = (ticks as f32 * speed * 0.02 + phase).sin() * amp;
                let new_x = origin_x + offset;
                if let Some(obj) = c.get_game_object_mut(id) {
                    obj.position.0 = new_x;
                }
            }
            s = st.lock().unwrap();

            // ── Cull hooks that have scrolled far behind the player ───────────
            // We track live_hooks ourselves — see NOT IN API note on State struct.
            let cutoff = s.px - VW * 1.5;
            let to_remove: Vec<String> = s.live_hooks.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + HOOK_R*2.0 < cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            for name in &to_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-2000.0, -2000.0);
                }
            }
            let to_remove_set: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
            let active_hook_removed = s.hooked && to_remove_set.contains(s.active_hook.as_str());
            s.live_hooks.retain(|n| !to_remove_set.contains(n.as_str()));
            for name in to_remove {
                s.pool_free.push(name);
            }

            // Unhook if the active hook was culled
            if active_hook_removed {
                s.hooked = false;
                s.active_hook = String::new();
                drop(s);
                c.run(Action::Hide { target: Target::name("rope") });
                s = st.lock().unwrap();
            }

