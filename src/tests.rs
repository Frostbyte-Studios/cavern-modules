use std::time::Duration;

use game_loop::winit::{platform::windows::EventLoopBuilderExtWindows, event_loop::EventLoopBuilder};

use crate::*;

#[test]
fn modules() {
    let mut modules = Modules::new();
    modules.add_module(TestModule1::default());
    modules.add_module(TestModule2::default());
    modules.start((), EventLoopBuilder::new().with_any_thread(true).build(), WindowBuilder::new(), 60, 0.1, |g| {
        thread::sleep(Duration::from_millis(550));

        g.game.1.exit.emit(&()).unwrap();

        thread::sleep(Duration::from_millis(1000));

        println!("Exiting Main");

        g.exit();
    }, |_| {}, |_, _,| {});
}

#[derive(Default)]
struct TestModule1 {
    handler1: EventHandler<String>,
    handler2: EventHandler<u32>,
}

impl Module for TestModule1 {
    fn run(self, exit: Exit, _update: Update, _hooks: Hooks) {
        loop {
            if exit.should_exit() {
                println!("Exiting 1");
                break;
            }

            // Should never fail in this example, so unwrapping is fine. You would want more sophisticated error handling in almost every other case.
            self.handler1.emit(&String::from("Test 1")).unwrap();
            thread::sleep(Duration::from_millis(25));
            self.handler1.emit(&String::from("Test 2")).unwrap();
            thread::sleep(Duration::from_millis(25));
            self.handler2.emit(&1).unwrap();
            thread::sleep(Duration::from_millis(25));
            self.handler2.emit(&2).unwrap();
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn hook(&mut self) -> Hook {
        let mut hook = Hook::new();

        hook.add("Messenger 1", self.handler1.clone());
        hook.add("Messenger 2", self.handler2.clone());

        hook
    }
}

#[derive(Default)]
struct TestModule2;

impl Module for TestModule2 {
    fn run(self, exit: Exit, _update: Update, hooks: Hooks) {
        let (handler1, handler2) = hooks
            .get::<TestModule1, _>(|hook| {
                (
                    hook.get::<String>("Messenger 1").unwrap().subscribe(),
                    hook.get::<u32>("Messenger 2").unwrap().subscribe(),
                )
            })
            .unwrap();

        loop {
            if exit.should_exit() {
                println!("Exiting 2");
                break;
            }

            for message in handler1.try_iter() {
                //println!("Received: {}", message);
                assert!(message == "Test 1" || message == "Test 2");
            }

            for message in handler2.try_iter() {
                //println!("Received: {}", message);
                assert!(message == 1 || message == 2);
            }

            thread::sleep(Duration::from_millis(100));
        }
    }

    fn hook(&mut self) -> Hook {
        Hook::new()
    }
}
