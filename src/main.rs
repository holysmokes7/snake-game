use std::ptr::addr_of_mut;
use std::time::Duration;

use rand::prelude::*;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

const FRAMES_PER_SECOND: f32 = 20.0;
const MOVE_SPEED: i32 = 8;
const ARENA_X: usize = 17;
const ARENA_Y: usize = 15;

const HEAD: i32 = 1;
const BODY: i32 = 2;
const FOOD: i32 = 3;

#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn from_key(key: u8) -> Result<Self, &'static str> {
        use Direction as Dir;
        match key {
            // W
            87 => Ok(Dir::Up),
            // S
            83 => Ok(Dir::Down),
            // A
            65 => Ok(Dir::Left),
            // D
            68 => Ok(Dir::Right),
            _ => Err("Invalid key"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Position {
    x: i32,
    y: i32,
}

struct Snake {
    direction: Direction,
    body: Vec<Position>,
}

impl Snake {
    /// Will move the snake body one space
    fn move_once(&mut self) {
        let old_pos = self.body.clone().into_iter();
        // move the head
        self.body[0] = Snake::next_pos(self.body[0], self.direction);

        // make the rest of the body follow
        self.body
            .iter_mut()
            .skip(1)
            .zip(old_pos)
            .for_each(|(old, new)| {
                *old = new;
            });
    }

    /// Will insert a new head to the snake
    fn add_new_head(&mut self) {
        self.body
            .insert(0, Snake::next_pos(*self.head_pos(), self.direction));
    }

    /// Returns the next position of a position based on the given direction
    fn next_pos(pos: Position, dir: Direction) -> Position {
        use Direction as Dir;
        let mut next_pos = match dir {
            Dir::Up => Position {
                x: pos.x,
                y: pos.y - 1,
            },
            Dir::Down => Position {
                x: pos.x,
                y: pos.y + 1,
            },
            Dir::Left => Position {
                x: pos.x - 1,
                y: pos.y,
            },
            Dir::Right => Position {
                x: pos.x + 1,
                y: pos.y,
            },
        };
        next_pos.x = wrap(next_pos.x, 0, ARENA_X as i32 - 1);
        next_pos.y = wrap(next_pos.y, 0, ARENA_Y as i32 - 1);
        next_pos
    }

    /// Update direction based on which key is being pressed and current direction
    fn update_direction(&mut self, key: u8) {
        let new_direction = match Direction::from_key(key) {
            Ok(direction) => direction,
            Err(_) => self.direction,
        };
        use Direction as Dir;
        // if new direction is opposite of current direction,
        // don't change current direction
        self.direction = match (self.direction, new_direction) {
            (Dir::Up, Dir::Down) => self.direction,
            (Dir::Down, Dir::Up) => self.direction,
            (Dir::Left, Dir::Right) => self.direction,
            (Dir::Right, Dir::Left) => self.direction,
            _ => new_direction,
        }
    }

    /// Get the position of the current head
    fn head_pos(&self) -> &Position {
        self.body.get(0).expect("Snake is empty")
    }

    /// Check if the snakes head is overlapping with any of the body segments
    fn should_be_dead(&self) -> bool {
        self.body.iter().skip(1).any(|pos| self.head_pos() == pos)
    }
}

fn main() {
    let std_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE).expect("Failed to get std handle") };

    // set cursor to be invisible, this only works 80% of the time
    let mut cursor_info: CONSOLE_CURSOR_INFO = Default::default();
    unsafe {
        GetConsoleCursorInfo(std_handle, addr_of_mut!(cursor_info))
            .expect("Couldn't get console cursor info");
        cursor_info.bVisible.0 = 0;
        SetConsoleCursorInfo(std_handle, addr_of_mut!(cursor_info))
            .expect("Couldn't set console cursor info");
    };

    let mut arena = [[0; ARENA_X]; ARENA_Y];
    // starting position for snake
    let mut snake = Snake {
        direction: Direction::Left,
        body: vec![
            Position { x: 4, y: 4 },
            Position { x: 4, y: 5 },
            Position { x: 5, y: 5 },
        ],
    };
    // starting position for food
    arena[12][10] = FOOD;
    arena[12][5] = FOOD;

    unsafe {
        // clear junk values
        for i in 0x1..=0xFE {
            GetAsyncKeyState(i);
        }
        let mut got_direction = false;
        let mut paused = false;
        'running: for i in (1..=FRAMES_PER_SECOND as i32 / MOVE_SPEED).cycle() {
            for key in 0x1u8..=0xFEu8 {
                if GetAsyncKeyState(key as i32) & 0b1 != 0 {
                    if !got_direction {
                        got_direction = true;
                        snake.update_direction(key);
                    }
                    if key == 32 {
                        // 32 is the space key
                        paused = match paused {
                            true => false,
                            false => {
                                print_paused(std_handle);
                                true
                            }
                        };
                    } else if key == 27 {
                        // 27 is the escape key
                        break 'running;
                    }
                }
            }
            // i == 1 will make it so the game doesn't update
            // a million times a second
            if i == 1 && !paused {
                let next_pos = Snake::next_pos(*snake.head_pos(), snake.direction);
                // need to check if the next space is food
                // so we can decide whether we should increase snake size
                // or just move forward
                if arena[next_pos.y as usize][next_pos.x as usize] == FOOD {
                    snake.add_new_head();
                    if arena.iter_mut().flatten().all(|num| *num != 0) {
                        break 'running;
                    }
                    spawn_food(&mut arena);
                } else {
                    snake.move_once();
                }
                update_arena(&mut arena, &snake);
                // render
                print_arena(std_handle, &arena);
                print_score(std_handle, &snake);
                // want to end game after last frame is rendered
                if snake.should_be_dead() {
                    break 'running;
                }
                got_direction = false;
            }
            std::thread::sleep(Duration::from_secs_f32(1.0 / FRAMES_PER_SECOND));
        }
    }
}

fn spawn_food(arena: &mut [[i32; ARENA_X]; ARENA_Y]) {
    let mut rng = thread_rng();
    let mut x = rng.gen_range(0..ARENA_X);
    let mut y = rng.gen_range(0..ARENA_Y);
    while arena[y][x] != 0 {
        x = rng.gen_range(0..ARENA_X);
        y = rng.gen_range(0..ARENA_Y);
    }
    arena[y][x] = FOOD;
}

fn update_arena(arena: &mut [[i32; ARENA_X]; ARENA_Y], snake: &Snake) {
    for i in arena.iter_mut().flatten() {
        if *i != FOOD {
            *i = 0;
        }
    }
    for (i, pos) in snake.body.iter().enumerate().rev() {
        if i == 0 {
            arena[pos.y as usize][pos.x as usize] = HEAD;
        } else {
            arena[pos.y as usize][pos.x as usize] = BODY;
        }
    }
}

fn print_arena(std_handle: HANDLE, arena: &[[i32; ARENA_X]; ARENA_Y]) {
    unsafe {
        for (i, row) in arena.iter().enumerate() {
            for (j, item) in row.iter().enumerate() {
                SetConsoleCursorPosition(
                    std_handle,
                    COORD {
                        X: j as i16 * 2,
                        Y: i as i16,
                    },
                )
                .expect("Failed to set console cursor position");
                let mut buf: [u8; 2] = [0; 2];
                buf[0] = match *item {
                    HEAD => b'1',
                    BODY => b'2',
                    FOOD => b'3',
                    _ => b'.',
                };
                buf[1] = b' ';
                WriteConsoleA(std_handle, &buf, None, None).expect("Failed to write to console");
            }
        }
    }
}

fn print_score(std_handle: HANDLE, snake: &Snake) {
    unsafe {
        SetConsoleCursorPosition(
            std_handle,
            COORD {
                X: 0,
                Y: ARENA_Y as i16,
            },
        )
        .expect("Failed to set console cursor position");
        let str = snake.body.len().to_string();
        let buf = str.as_bytes();
        WriteConsoleA(std_handle, buf, None, None).expect("Failed to write to console");
    }
}

fn print_paused(std_handle: HANDLE) {
    unsafe {
        SetConsoleCursorPosition(
            std_handle,
            COORD {
                X: ARENA_X as i16 - 6,
                Y: ARENA_Y as i16 / 3,
            },
        )
        .expect("Failed to set console cursor position");
        WriteConsoleA(std_handle, "P A U S E D ".as_bytes(), None, None)
            .expect("Failed to write to console");
    }
}

fn wrap(val: i32, min: i32, max: i32) -> i32 {
    if val < min {
        max
    } else if val > max {
        min
    } else {
        val
    }
}
