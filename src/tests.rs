use std::{thread, time::Duration};

use crate::{Modules, Module, hook::{Hooks, Hook}};

#[test]
fn test() {
    let modules = Modules::new();

    modules.start((), 60, 0.1, |_game_loop| {
        // Update (60 times per second)
    }, |game_loop| {
        // Render (As fast as possible)

        // For the test, just sleep for a short amount of time and then exit
        thread::sleep(Duration::from_millis(100));
        game_loop.game.1.exit.emit(&()).unwrap();
        game_loop.exit()
    });
}

struct ExampleModule;

impl Module for ExampleModule {
    fn start(&mut self, _hooks: &Hooks) {
        // When start is called on modules
    }

    fn render(&mut self, _hooks: &Hooks) {
        // Render (As fast as possible)
    }

    fn update(&mut self, _hooks: &Hooks) {
        // Update (60 times per second)
    }

    fn hook(&mut self) -> Hook {
        Hook::new(())
    }
}