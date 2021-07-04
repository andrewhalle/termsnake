use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

use termion::clear;
use termion::color;
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen;
use termion::{get_tty, terminal_size};

use rand::prelude::*;

type TermCoord = (u16, u16);

struct Game {
    term: RawTerminal<File>,
    snake: VecDeque<TermCoord>,
    last_key: Key,
    food: TermCoord,
    bounds: TermCoord,
    events: Receiver<Key>,
}

impl Game {
    fn new() -> Self {
        let (tx, rx) = channel();

        // have to make a channel and send key events over it so that we don't block the main loop
        thread::spawn(move || {
            for key in get_tty().unwrap().keys() {
                tx.send(key.unwrap()).unwrap();
            }
        });

        let bounds = terminal_size().unwrap();
        let mut term = get_tty().unwrap().into_raw_mode().unwrap();
        write!(
            term,
            "{}{}{}",
            screen::ToAlternateScreen,
            clear::All,
            cursor::Hide
        )
        .unwrap();
        let snake = vec![(bounds.0 / 2, bounds.1 / 2)].into();
        let mut game = Game {
            term,
            food: Game::generate_food_pos(bounds, &snake),
            snake,
            last_key: Key::Right,
            bounds,
            events: rx,
        };

        // draw initial state
        game.ink(game.snake[0], &color::Red);
        game.ink(game.food, &color::Green);

        game
    }

    fn ink(&mut self, pos: TermCoord, color: &dyn color::Color) {
        write!(
            self.term,
            "{}{}{} {}",
            cursor::Save,
            cursor::Goto(pos.0, pos.1),
            color::Bg(color),
            cursor::Restore
        )
        .unwrap()
    }

    fn de_ink(&mut self, pos: TermCoord) {
        write!(self.term, "{} ", cursor::Goto(pos.0, pos.1)).unwrap()
    }

    fn generate_food_pos(bounds: TermCoord, snake: &VecDeque<TermCoord>) -> TermCoord {
        let mut rng = rand::thread_rng();
        let mut food = (
            rng.gen_range(10..bounds.0 - 10),
            rng.gen_range(10..bounds.1 - 10),
        );
        while snake.contains(&food) {
            food = (
                rng.gen_range(10..bounds.0 - 10),
                rng.gen_range(10..bounds.1 - 10),
            );
        }

        food
    }

    fn opposite(key: Key) -> Key {
        use Key::*;

        match key {
            Up => Down,
            Down => Up,
            Left => Right,
            Right => Left,
            // unreachable because this is only called on Game::last_key, which is already checked
            _ => unreachable!(),
        }
    }

    fn check_valid(key: Key) -> Result<(), ()> {
        use Key::*;

        match key {
            Up | Down | Right | Left | Char('h' | 'j' | 'k' | 'l') => Ok(()),
            _ => Err(()),
        }
    }

    fn as_direction_key(key: Key) -> Key {
        use Key::*;

        match key {
            Up | Char('k') => Up,
            Down | Char('j') => Down,
            Left | Char('h') => Left,
            Right | Char('l') => Right,
            // unreachable because we've already gone through check_valid
            _ => unreachable!(),
        }
    }

    fn handle_key(&mut self, key: Key) -> Result<(), ()> {
        Game::check_valid(key)?;
        let opposite = Game::opposite(self.last_key);
        let direction = Game::as_direction_key(key);

        if !(direction == opposite) {
            self.last_key = direction;
        }

        Ok(())
    }

    fn valid_head(&self, new_head: TermCoord) -> Result<(), ()> {
        let invalid = new_head.0 >= self.bounds.0
            || new_head.1 >= self.bounds.1
            || self.snake.contains(&new_head);

        if invalid {
            Err(())
        } else {
            Ok(())
        }
    }

    fn update(&mut self) -> Result<(), ()> {
        let old_head = self.snake.front().unwrap().to_owned();
        let mut new_head = old_head;

        if new_head.0 == 0 || new_head.1 == 0 {
            return Err(());
        }

        match self.last_key {
            Key::Up => new_head.1 = new_head.1.checked_sub(1).ok_or(())?,
            Key::Down => new_head.1 += 1,
            Key::Left => new_head.0 = new_head.0.checked_sub(1).ok_or(())?,
            Key::Right => new_head.0 += 1,
            _ => unreachable!(),
        }

        if new_head == old_head {
            return Ok(());
        }

        self.ink(old_head, &color::Blue);
        self.valid_head(new_head)?;
        self.ink(new_head, &color::Red);
        self.snake.push_front(new_head);

        if *self.snake.front().unwrap() == self.food {
            self.food = Game::generate_food_pos(self.bounds, &self.snake);
            self.ink(self.food, &color::Green);
        } else {
            let old_tail = self.snake.pop_back().unwrap();
            self.de_ink(old_tail);
        }

        Ok(())
    }

    fn vertical(&self) -> bool {
        matches!(self.last_key, Key::Up | Key::Down)
    }

    fn game_loop(&mut self) -> Result<(), ()> {
        loop {
            match self.events.try_recv() {
                // an Err here indicates that no key is available
                Err(_) => {}
                Ok(key) => {
                    self.handle_key(key)?;
                }
            }

            self.update()?;

            // different values for vertical and horizontal motion because most terminals have a
            // cell size that is taller than it is wide.
            thread::sleep(Duration::from_millis(if self.vertical() { 70 } else { 50 }));
        }
    }
}

fn main() {
    let mut game = Game::new();

    let _ = game.game_loop();

    write!(game.term, "{}{}", cursor::Show, screen::ToMainScreen).unwrap();
    game.term.suspend_raw_mode().unwrap();
    println!("Score: {}", game.snake.len());
}
