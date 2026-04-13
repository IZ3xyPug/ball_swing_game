    .on_exit(|canvas| {
        canvas.run(Action::DetachEmitter {
            emitter_name: PLAYER_TRAIL_EMITTER_NAME.to_string(),
        });
        canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
    })
}
