use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{ExecutableCommand, QueueableCommand};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::VecDeque;
use std::io::{self, Stdout, Write};
use std::thread;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

const DEFAULT_GRID_W: usize = 31;
const DEFAULT_GRID_H: usize = 21;
const PEN_W: usize = 9;
const PEN_H: usize = 5;
const GHOST_RELEASE_INTERVAL: u32 = 90;
const BONUS_MIN_TICKS: u32 = 600;
const BONUS_MAX_TICKS: u32 = 1100;
const BONUS_LIFETIME_TICKS: u32 = 260;
const BONUS_SCORE: u32 = 200;
const BONUS_POWER_BOOST: u32 = 40;
const CELL_W: usize = 2;
const DEFAULT_TICK_MS: u64 = 70;
const POWER_TICKS: u32 = 90;
const DEFAULT_RENDER_FPS: u64 = 120;
const BRAID_CHANCE: f32 = 0.45;
const EXTRA_OPENINGS: f32 = 0.08;
const INPUT_HOLD_MS: u64 = 160;
const GHOST_MOVE_INTERVAL: u32 = 2;

#[derive(Clone, Copy, PartialEq)]
enum Tile {
    Wall,
    Empty,
    Pellet,
    Power,
    Gate,
}

#[derive(Clone, Copy, PartialEq)]
struct Pos {
    x: usize,
    y: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn delta(self) -> (isize, isize) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }
}

struct Game {
    width: usize,
    height: usize,
    grid: Vec<Vec<Tile>>,
    player: Pos,
    player_spawn: Pos,
    ghosts: Vec<Pos>,
    ghost_spawns: Vec<Pos>,
    score: u32,
    lives: u32,
    level: u32,
    pellets_left: usize,
    power_timer: u32,
    dir: Option<Dir>,
    ghost_tick: u32,
    ghost_release: Vec<u32>,
    pen_bounds: PenBounds,
    bonus_pos: Option<Pos>,
    bonus_timer: u32,
    bonus_spawn_in: u32,
}

impl Game {
    fn apply_input(&mut self, desired_dir: Option<Dir>, input_active: bool) {
        if !input_active {
            self.dir = None;
        } else if let Some(dir) = desired_dir {
            if can_move_player(&self.grid, self.width, self.height, self.player, dir) {
                self.dir = Some(dir);
            }
        }
    }

    fn move_player(&mut self) {
        if let Some(dir) = self.dir {
            if can_move_player(&self.grid, self.width, self.height, self.player, dir) {
                self.player = step(self.player, dir);
            } else {
                self.dir = None;
            }
        }
    }

    fn consume_tile(&mut self) {
        match self.grid[self.player.y][self.player.x] {
            Tile::Pellet => {
                self.grid[self.player.y][self.player.x] = Tile::Empty;
                self.score += 10;
                self.pellets_left = self.pellets_left.saturating_sub(1);
            }
            Tile::Power => {
                self.grid[self.player.y][self.player.x] = Tile::Empty;
                self.score += 50;
                self.pellets_left = self.pellets_left.saturating_sub(1);
                self.power_timer = POWER_TICKS;
            }
            _ => {}
        }
    }

    fn try_collect_bonus(&mut self, rng: &mut impl Rng) {
        if let Some(pos) = self.bonus_pos {
            if pos == self.player {
                self.score += BONUS_SCORE;
                self.power_timer = (self.power_timer + BONUS_POWER_BOOST).max(BONUS_POWER_BOOST);
                self.bonus_pos = None;
                self.bonus_timer = 0;
                self.bonus_spawn_in = rng.gen_range(BONUS_MIN_TICKS..=BONUS_MAX_TICKS);
            }
        }
    }

    fn update_bonus(&mut self, rng: &mut impl Rng) {
        if self.bonus_pos.is_some() {
            if self.bonus_timer > 0 {
                self.bonus_timer -= 1;
            } else {
                self.bonus_pos = None;
                self.bonus_spawn_in = rng.gen_range(BONUS_MIN_TICKS..=BONUS_MAX_TICKS);
            }
        } else if self.bonus_spawn_in > 0 {
            self.bonus_spawn_in -= 1;
        } else {
            if let Some(pos) = random_bonus_spawn(self, rng) {
                self.bonus_pos = Some(pos);
                self.bonus_timer = BONUS_LIFETIME_TICKS;
            }
            self.bonus_spawn_in = rng.gen_range(BONUS_MIN_TICKS..=BONUS_MAX_TICKS);
        }
    }

    fn update_ghosts(&mut self, rng: &mut impl Rng) {
        self.ghost_tick = self.ghost_tick.wrapping_add(1);
        if self.ghost_tick % GHOST_MOVE_INTERVAL != 0 {
            return;
        }
        let dist = bfs_distance(&self.grid, self.width, self.height, self.player, true);
        for (idx, ghost) in self.ghosts.iter_mut().enumerate() {
            if self.ghost_release[idx] > 0 {
                self.ghost_release[idx] = self.ghost_release[idx].saturating_sub(1);
                let dir = ghost_next_dir_pen(
                    *ghost,
                    &self.grid,
                    self.width,
                    self.height,
                    &self.pen_bounds,
                    rng,
                );
                if let Some(dir) = dir {
                    *ghost = step(*ghost, dir);
                }
                continue;
            }
            let dir =
                ghost_next_dir(*ghost, &self.grid, self.width, self.height, &dist, rng, true);
            if let Some(dir) = dir {
                *ghost = step(*ghost, dir);
            }
        }
    }

    fn tick_power_timer(&mut self) {
        if self.power_timer > 0 {
            self.power_timer -= 1;
        }
    }

    fn handle_collisions(&mut self, rng: &mut impl Rng) {
        let mut hit = None;
        for (idx, ghost) in self.ghosts.iter().enumerate() {
            if *ghost == self.player {
                hit = Some(idx);
                break;
            }
        }

        if let Some(idx) = hit {
            if self.power_timer > 0 {
                self.score += 200;
                self.ghosts[idx] = self.ghost_spawns[idx];
            } else {
                if self.lives > 0 {
                    self.lives -= 1;
                }
                self.player = self.player_spawn;
                self.ghosts = self.ghost_spawns.clone();
                self.ghost_release.clear();
                for i in 0..self.ghost_spawns.len() {
                    self.ghost_release.push(i as u32 * GHOST_RELEASE_INTERVAL);
                }
                self.power_timer = 0;
                self.bonus_pos = None;
                self.bonus_timer = 0;
                self.bonus_spawn_in = rng.gen_range(BONUS_MIN_TICKS..=BONUS_MAX_TICKS);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Glyph {
    Player,
    Ghost,
    Frightened,
    Wall,
    Empty,
    Pellet,
    Power,
    Gate,
    Bonus,
}

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    glyph: Glyph,
    color: Color,
}

#[derive(Clone, Copy)]
struct PenBounds {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

struct Renderer {
    last: Vec<Cell>,
    last_hud: String,
    needs_full: bool,
    origin_x: u16,
    origin_y: u16,
}

impl Renderer {
    fn new(width: usize, height: usize) -> Self {
        Self {
            last: vec![
                Cell {
                    glyph: Glyph::Empty,
                    color: Color::Reset,
                };
                width * height
            ],
            last_hud: String::new(),
            needs_full: true,
            origin_x: 0,
            origin_y: 1,
        }
    }
}

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(Hide)?;

    let result = run(&mut stdout);

    stdout.execute(Show)?;
    stdout.execute(LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    result
}

fn run(stdout: &mut Stdout) -> io::Result<()> {
    let mut rng = rand::thread_rng();
    let grid_w = DEFAULT_GRID_W;
    let grid_h = DEFAULT_GRID_H;
    let mut game = new_game(&mut rng, 1, grid_w, grid_h);
    let mut last_tick = Instant::now();
    let mut last_seen: [Option<Instant>; 4] = [None, None, None, None];
    let mut last_pressed: Option<Dir> = None;
    let mut renderer = Renderer::new(grid_w, grid_h);
    let (tick_ms, render_fps) = read_speed_settings();
    let frame_time = Duration::from_micros(1_000_000 / render_fps.max(1));

    loop {
        let frame_start = Instant::now();
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                match key.kind {
                    KeyEventKind::Press | KeyEventKind::Repeat => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('k') => {
                            last_seen[0] = Some(Instant::now());
                            last_pressed = Some(Dir::Up);
                        }
                        KeyCode::Char('j') => {
                            last_seen[1] = Some(Instant::now());
                            last_pressed = Some(Dir::Down);
                        }
                        KeyCode::Char('h') => {
                            last_seen[2] = Some(Instant::now());
                            last_pressed = Some(Dir::Left);
                        }
                        KeyCode::Char('l') => {
                            last_seen[3] = Some(Instant::now());
                            last_pressed = Some(Dir::Right);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= Duration::from_millis(tick_ms) {
            last_tick = Instant::now();
            let desired_dir = active_dir_recent(&last_seen, last_pressed);
            let input_active = desired_dir.is_some();
            tick(&mut game, &mut rng, desired_dir, input_active);
            render(stdout, &mut game, &mut renderer)?;
            if game.lives == 0 {
                render_game_over(stdout, &game)?;
                return Ok(());
            }
        } else {
            render(stdout, &mut game, &mut renderer)?;
        }

        let elapsed = frame_start.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
    }
}

fn read_speed_settings() -> (u64, u64) {
    let tick_ms = std::env::var("PACMAN_TICK_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_TICK_MS);
    let render_fps = std::env::var("PACMAN_FPS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_RENDER_FPS);
    (tick_ms, render_fps)
}

fn new_game(rng: &mut impl Rng, level: u32, width: usize, height: usize) -> Game {
    let (grid, pellets_left, ghost_spawns, pen_bounds) = generate_maze(rng, width, height);
    let mut empties = empty_cells(&grid);
    empties.shuffle(rng);
    let player = empties
        .iter()
        .copied()
        .find(|p| !ghost_spawns.contains(p) && !is_in_pen(*p, width, height))
        .expect("maze has empty cells");
    let player_spawn = player;

    let mut ghost_release = Vec::new();
    for i in 0..ghost_spawns.len() {
        ghost_release.push(i as u32 * GHOST_RELEASE_INTERVAL);
    }

    let bonus_spawn_in = rng.gen_range(BONUS_MIN_TICKS..=BONUS_MAX_TICKS);
    Game {
        width,
        height,
        grid,
        player,
        player_spawn,
        ghosts: ghost_spawns.clone(),
        ghost_spawns,
        score: 0,
        lives: 3,
        level,
        pellets_left,
        power_timer: 0,
        dir: None,
        ghost_tick: 0,
        ghost_release,
        pen_bounds,
        bonus_pos: None,
        bonus_timer: 0,
        bonus_spawn_in,
    }
}

fn next_level(game: &mut Game, rng: &mut impl Rng) {
    game.level += 1;
    let (grid, pellets_left, ghost_spawns, pen_bounds) = generate_maze(rng, game.width, game.height);
    let mut empties = empty_cells(&grid);
    empties.shuffle(rng);
    game.grid = grid;
    game.pellets_left = pellets_left;
    game.player = empties
        .iter()
        .copied()
        .find(|p| !ghost_spawns.contains(p) && !is_in_pen(*p, game.width, game.height))
        .expect("maze has empty cells");
    game.player_spawn = game.player;
    game.ghost_spawns = ghost_spawns;
    game.ghosts = game.ghost_spawns.clone();
    game.ghost_release.clear();
    for i in 0..game.ghost_spawns.len() {
        game.ghost_release.push(i as u32 * GHOST_RELEASE_INTERVAL);
    }
    game.pen_bounds = pen_bounds;
    game.power_timer = 0;
    game.dir = None;
    game.ghost_tick = 0;
    game.bonus_pos = None;
    game.bonus_timer = 0;
    game.bonus_spawn_in = rng.gen_range(BONUS_MIN_TICKS..=BONUS_MAX_TICKS);
}

fn tick(game: &mut Game, rng: &mut impl Rng, desired_dir: Option<Dir>, input_active: bool) {
    game.apply_input(desired_dir, input_active);
    game.move_player();
    game.consume_tile();
    game.try_collect_bonus(rng);

    if game.pellets_left == 0 {
        next_level(game, rng);
        return;
    }

    game.update_bonus(rng);
    game.update_ghosts(rng);
    game.tick_power_timer();
    game.handle_collisions(rng);
}

fn render(stdout: &mut Stdout, game: &mut Game, renderer: &mut Renderer) -> io::Result<()> {
    let needed_h = (game.height + 2) as u16;
    let needed_w = (game.width * CELL_W) as u16;

    stdout.queue(MoveTo(0, 0))?;

    let (term_w, term_h) = terminal::size()?;
    if term_w < needed_w || term_h < needed_h {
        stdout.queue(Clear(ClearType::All))?;
        let msg = format!(
            "Terminal too small. Need at least {}x{} (cols x rows). Current: {}x{}.",
            needed_w, needed_h, term_w, term_h
        );
        stdout.queue(Print(msg))?;
        stdout.flush()?;
        renderer.needs_full = true;
        return Ok(());
    }

    let origin_x = (term_w - needed_w) / 2;
    let origin_y = (term_h - needed_h) / 2 + 1;
    if origin_x != renderer.origin_x || origin_y != renderer.origin_y {
        renderer.origin_x = origin_x;
        renderer.origin_y = origin_y;
        renderer.needs_full = true;
    }

    let hud = format!(
        "Score: {}  Lives: {}  Level: {}  Pellets: {}  Power: {}  (q to quit)",
        game.score, game.lives, game.level, game.pellets_left, game.power_timer
    );
    if renderer.needs_full || hud != renderer.last_hud {
        stdout.queue(MoveTo(renderer.origin_x, renderer.origin_y - 1))?;
        stdout.queue(SetForegroundColor(Color::White))?;
        stdout.queue(Clear(ClearType::CurrentLine))?;
        stdout.queue(Print(&hud))?;
        stdout.queue(ResetColor)?;
        renderer.last_hud = hud;
    }

    for y in 0..game.height {
        for x in 0..game.width {
            let pos = Pos { x, y };
            let cell = cell_for(game, pos);
            let idx = y * game.width + x;
            if renderer.needs_full || cell != renderer.last[idx] {
                renderer.last[idx] = cell;
                draw_cell(stdout, renderer, x, y, cell)?;
            }
        }
    }
    renderer.needs_full = false;

    stdout.flush()?;
    Ok(())
}

fn cell_for(game: &Game, pos: Pos) -> Cell {
    if pos == game.player {
        return Cell {
            glyph: Glyph::Player,
            color: Color::Yellow,
        };
    }
    if game.ghosts.iter().any(|g| *g == pos) {
        if game.power_timer > 0 {
            return Cell {
                glyph: Glyph::Frightened,
                color: Color::Blue,
            };
        }
        return Cell {
            glyph: Glyph::Ghost,
            color: Color::Red,
        };
    }
    if game.bonus_pos == Some(pos) {
        return Cell {
            glyph: Glyph::Bonus,
            color: Color::Green,
        };
    }
    match game.grid[pos.y][pos.x] {
        Tile::Wall => Cell {
            glyph: Glyph::Wall,
            color: Color::Blue,
        },
        Tile::Gate => Cell {
            glyph: Glyph::Gate,
            color: Color::Cyan,
        },
        Tile::Empty => Cell {
            glyph: Glyph::Empty,
            color: Color::Reset,
        },
        Tile::Pellet => Cell {
            glyph: Glyph::Pellet,
            color: Color::White,
        },
        Tile::Power => Cell {
            glyph: Glyph::Power,
            color: Color::Magenta,
        },
    }
}

fn draw_cell(stdout: &mut Stdout, renderer: &Renderer, x: usize, y: usize, cell: Cell) -> io::Result<()> {
    let (text, color) = match cell.glyph {
        Glyph::Player => ("😃", cell.color),
        Glyph::Ghost => ("👻", cell.color),
        Glyph::Frightened => ("😱", cell.color),
        Glyph::Wall => ("██", cell.color),
        Glyph::Empty => ("  ", cell.color),
        Glyph::Pellet => ("· ", cell.color),
        Glyph::Power => ("● ", cell.color),
        Glyph::Gate => ("==", cell.color),
        Glyph::Bonus => ("🍒", cell.color),
    };
    let x_pos = renderer.origin_x + (x * CELL_W) as u16;
    let y_pos = renderer.origin_y + y as u16;
    stdout.queue(MoveTo(x_pos, y_pos))?;
    stdout.queue(SetForegroundColor(color))?;
    stdout.queue(Print(text))?;
    let w = UnicodeWidthStr::width(text);
    if w < CELL_W {
        for _ in 0..(CELL_W - w) {
            stdout.queue(Print(' '))?;
        }
    }
    stdout.queue(ResetColor)?;
    Ok(())
}

fn render_game_over(stdout: &mut Stdout, game: &Game) -> io::Result<()> {
    let (term_w, term_h) = terminal::size()?;
    let needed_h = (game.height + 2) as u16;
    let needed_w = (game.width * CELL_W) as u16;
    if term_w < needed_w || term_h < needed_h {
        stdout.queue(MoveTo(0, needed_h))?;
    } else {
        let origin_x = (term_w - needed_w) / 2;
        let origin_y = (term_h - needed_h) / 2 + 1;
        stdout.queue(MoveTo(origin_x, origin_y + game.height as u16))?;
    }
    stdout.queue(Print(format!(
        "GAME OVER - Final Score: {} (press q to quit)",
        game.score
    )))?;
    stdout.flush()?;
    loop {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    return Ok(());
                }
            }
        }
    }
}

fn active_dir_recent(last_seen: &[Option<Instant>; 4], last_pressed: Option<Dir>) -> Option<Dir> {
    let now = Instant::now();
    if let Some(dir) = last_pressed {
        if let Some(t) = last_seen[idx_for_dir(dir)] {
            if now.duration_since(t) <= Duration::from_millis(INPUT_HOLD_MS) {
                return Some(dir);
            }
        }
    }
    let mut best: Option<(Dir, Instant)> = None;
    for (idx, dir) in [Dir::Up, Dir::Down, Dir::Left, Dir::Right].iter().enumerate() {
        if let Some(t) = last_seen[idx] {
            if now.duration_since(t) <= Duration::from_millis(INPUT_HOLD_MS) {
                match best {
                    None => best = Some((*dir, t)),
                    Some((_, bt)) if t > bt => best = Some((*dir, t)),
                    _ => {}
                }
            }
        }
    }
    best.map(|(dir, _)| dir)
}

fn idx_for_dir(dir: Dir) -> usize {
    match dir {
        Dir::Up => 0,
        Dir::Down => 1,
        Dir::Left => 2,
        Dir::Right => 3,
    }
}

fn empty_cells(grid: &[Vec<Tile>]) -> Vec<Pos> {
    let mut cells = Vec::new();
    for y in 0..grid.len() {
        for x in 0..grid[y].len() {
            if grid[y][x] != Tile::Wall && grid[y][x] != Tile::Gate {
                cells.push(Pos { x, y });
            }
        }
    }
    cells
}

fn can_move_player(grid: &[Vec<Tile>], width: usize, height: usize, pos: Pos, dir: Dir) -> bool {
    let (dx, dy) = dir.delta();
    let nx = pos.x as isize + dx;
    let ny = pos.y as isize + dy;
    if nx < 0 || ny < 0 {
        return false;
    }
    let nx = nx as usize;
    let ny = ny as usize;
    if nx >= width || ny >= height {
        return false;
    }
    match grid[ny][nx] {
        Tile::Wall | Tile::Gate => false,
        _ => true,
    }
}

fn can_move_ghost(
    grid: &[Vec<Tile>],
    width: usize,
    height: usize,
    pos: Pos,
    dir: Dir,
    gate_open: bool,
) -> bool {
    let (dx, dy) = dir.delta();
    let nx = pos.x as isize + dx;
    let ny = pos.y as isize + dy;
    if nx < 0 || ny < 0 {
        return false;
    }
    let nx = nx as usize;
    let ny = ny as usize;
    if nx >= width || ny >= height {
        return false;
    }
    match grid[ny][nx] {
        Tile::Wall => false,
        Tile::Gate => gate_open,
        _ => true,
    }
}

fn step(pos: Pos, dir: Dir) -> Pos {
    let (dx, dy) = dir.delta();
    Pos {
        x: (pos.x as isize + dx) as usize,
        y: (pos.y as isize + dy) as usize,
    }
}

fn bfs_distance(
    grid: &[Vec<Tile>],
    width: usize,
    height: usize,
    start: Pos,
    gate_open: bool,
) -> Vec<Vec<i32>> {
    let mut dist = vec![vec![-1; width]; height];
    let mut q = VecDeque::new();
    dist[start.y][start.x] = 0;
    q.push_back(start);

    while let Some(pos) = q.pop_front() {
        let base = dist[pos.y][pos.x];
        for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
            if !can_move_ghost(grid, width, height, pos, dir, gate_open) {
                continue;
            }
            let next = step(pos, dir);
            if dist[next.y][next.x] == -1 {
                dist[next.y][next.x] = base + 1;
                q.push_back(next);
            }
        }
    }
    dist
}

fn ghost_next_dir(
    pos: Pos,
    grid: &[Vec<Tile>],
    width: usize,
    height: usize,
    dist: &[Vec<i32>],
    rng: &mut impl Rng,
    gate_open: bool,
) -> Option<Dir> {
    let mut options = Vec::new();
    let mut best = i32::MAX;
    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        if !can_move_ghost(grid, width, height, pos, dir, gate_open) {
            continue;
        }
        let next = step(pos, dir);
        let d = dist[next.y][next.x];
        if d >= 0 && d < best {
            best = d;
            options.clear();
            options.push(dir);
        } else if d >= 0 && d == best {
            options.push(dir);
        }
    }
    if options.is_empty() {
        None
    } else {
        Some(*options.choose(rng).unwrap())
    }
}

fn generate_maze(
    rng: &mut impl Rng,
    width: usize,
    height: usize,
) -> (Vec<Vec<Tile>>, usize, Vec<Pos>, PenBounds) {
    let mut grid = vec![vec![Tile::Wall; width]; height];
    let cells_w = (width - 1) / 2;
    let cells_h = (height - 1) / 2;
    let mut in_maze = vec![vec![false; cells_w]; cells_h];
    let mut frontier: Vec<(usize, usize)> = Vec::new();

    let start = (rng.gen_range(0..cells_w), rng.gen_range(0..cells_h));
    in_maze[start.1][start.0] = true;
    carve_cell(&mut grid, start.0, start.1);
    add_frontier(start.0, start.1, cells_w, cells_h, &in_maze, &mut frontier);

    while !frontier.is_empty() {
        let idx = rng.gen_range(0..frontier.len());
        let (cx, cy) = frontier.swap_remove(idx);
        if in_maze[cy][cx] {
            continue;
        }

        let mut neighbors = Vec::new();
        if cy > 0 && in_maze[cy - 1][cx] {
            neighbors.push((cx, cy - 1));
        }
        if cy + 1 < cells_h && in_maze[cy + 1][cx] {
            neighbors.push((cx, cy + 1));
        }
        if cx > 0 && in_maze[cy][cx - 1] {
            neighbors.push((cx - 1, cy));
        }
        if cx + 1 < cells_w && in_maze[cy][cx + 1] {
            neighbors.push((cx + 1, cy));
        }

        if neighbors.is_empty() {
            continue;
        }

        let (nx, ny) = *neighbors.choose(rng).unwrap();
        in_maze[cy][cx] = true;
        carve_between(&mut grid, cx, cy, nx, ny);
        carve_cell(&mut grid, cx, cy);
        add_frontier(cx, cy, cells_w, cells_h, &in_maze, &mut frontier);
    }

    braid_maze(&mut grid, cells_w, cells_h, rng);

    let (pen_all, _door, pen_spawns, pen_bounds) = carve_ghost_pen(&mut grid, width, height);
    ensure_connected(&mut grid, width, height, &pen_bounds);

    let mut pellets = 0;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            if grid[y][x] == Tile::Empty && !pen_all.iter().any(|p| p.x == x && p.y == y) {
                grid[y][x] = Tile::Pellet;
                pellets += 1;
            }
        }
    }

    let power_spots = [
        Pos { x: 1, y: 1 },
        Pos { x: width - 2, y: 1 },
        Pos { x: 1, y: height - 2 },
        Pos { x: width - 2, y: height - 2 },
    ];
    for pos in power_spots {
        if grid[pos.y][pos.x] != Tile::Wall {
            grid[pos.y][pos.x] = Tile::Power;
        }
    }

    // Ensure pen cells have no pellets (keep the gate intact).
    for pos in &pen_all {
        if grid[pos.y][pos.x] == Tile::Gate {
            continue;
        }
        if grid[pos.y][pos.x] != Tile::Wall {
            grid[pos.y][pos.x] = Tile::Empty;
        }
    }

    let ghost_spawns = pick_ghost_spawns(&pen_spawns);
    (grid, pellets, ghost_spawns, pen_bounds)
}

fn add_frontier(
    cx: usize,
    cy: usize,
    cells_w: usize,
    cells_h: usize,
    in_maze: &[Vec<bool>],
    frontier: &mut Vec<(usize, usize)>,
) {
    if cy > 0 && !in_maze[cy - 1][cx] {
        frontier.push((cx, cy - 1));
    }
    if cy + 1 < cells_h && !in_maze[cy + 1][cx] {
        frontier.push((cx, cy + 1));
    }
    if cx > 0 && !in_maze[cy][cx - 1] {
        frontier.push((cx - 1, cy));
    }
    if cx + 1 < cells_w && !in_maze[cy][cx + 1] {
        frontier.push((cx + 1, cy));
    }
}

fn carve_cell(grid: &mut [Vec<Tile>], cx: usize, cy: usize) {
    let gx = cx * 2 + 1;
    let gy = cy * 2 + 1;
    grid[gy][gx] = Tile::Empty;
}

fn carve_between(grid: &mut [Vec<Tile>], cx: usize, cy: usize, nx: usize, ny: usize) {
    let gx = cx * 2 + 1;
    let gy = cy * 2 + 1;
    let ngx = nx * 2 + 1;
    let ngy = ny * 2 + 1;
    let wall_x = (gx + ngx) / 2;
    let wall_y = (gy + ngy) / 2;
    grid[wall_y][wall_x] = Tile::Empty;
}

fn carve_ghost_pen(
    grid: &mut [Vec<Tile>],
    width: usize,
    height: usize,
) -> (Vec<Pos>, Pos, Vec<Pos>, PenBounds) {
    let (x0, y0, x1, y1) = pen_bounds(width, height);

    let mut pen_all = Vec::new();
    let mut pen_spawns = Vec::new();

    for y in y0..=y1 {
        for x in x0..=x1 {
            if y == y0 || y == y1 || x == x0 || x == x1 {
                grid[y][x] = Tile::Wall;
            } else {
                grid[y][x] = Tile::Empty;
                pen_all.push(Pos { x, y });
                pen_spawns.push(Pos { x, y });
            }
        }
    }

    let door_x = (x0 + x1) / 2;
    let door = Pos { x: door_x, y: y0 };
    grid[door.y][door.x] = Tile::Gate;
    pen_all.push(door);

    // Carve a vertical corridor from the gate upward until we hit open space,
    // guaranteeing connectivity between the pen and the maze.
    let mut y = door.y.saturating_sub(1);
    while y > 0 {
        if grid[y][door.x] != Tile::Wall {
            break;
        }
        grid[y][door.x] = Tile::Empty;
        y = y.saturating_sub(1);
    }

    (
        pen_all,
        door,
        pen_spawns,
        PenBounds { x0, y0, x1, y1 },
    )
}

fn pick_ghost_spawns(pen_spawns: &[Pos]) -> Vec<Pos> {
    let mut spawns = Vec::new();
    if pen_spawns.is_empty() {
        return spawns;
    }
    for pos in pen_spawns.iter().take(4) {
        spawns.push(*pos);
    }
    while spawns.len() < 4 {
        spawns.push(pen_spawns[0]);
    }
    spawns
}

fn pen_bounds(width: usize, height: usize) -> (usize, usize, usize, usize) {
    let mut pen_w = PEN_W.min(width.saturating_sub(2));
    let mut pen_h = PEN_H.min(height.saturating_sub(2));
    if pen_w % 2 == 0 {
        pen_w = pen_w.saturating_sub(1);
    }
    if pen_h % 2 == 0 {
        pen_h = pen_h.saturating_sub(1);
    }
    pen_w = pen_w.max(3);
    pen_h = pen_h.max(3);

    let x0 = (width - pen_w) / 2;
    let y0 = (height - pen_h) / 2;
    let x1 = x0 + pen_w - 1;
    let y1 = y0 + pen_h - 1;
    (x0, y0, x1, y1)
}

fn is_in_pen(pos: Pos, width: usize, height: usize) -> bool {
    let (x0, y0, x1, y1) = pen_bounds(width, height);
    pos.x >= x0 && pos.x <= x1 && pos.y >= y0 && pos.y <= y1
}

fn in_pen_interior(pos: Pos, pen: &PenBounds) -> bool {
    pos.x > pen.x0 && pos.x < pen.x1 && pos.y > pen.y0 && pos.y < pen.y1
}

fn is_pen_wall(pos: Pos, pen: &PenBounds) -> bool {
    (pos.x >= pen.x0 && pos.x <= pen.x1 && (pos.y == pen.y0 || pos.y == pen.y1))
        || (pos.y >= pen.y0 && pos.y <= pen.y1 && (pos.x == pen.x0 || pos.x == pen.x1))
}

fn ensure_connected(grid: &mut [Vec<Tile>], width: usize, height: usize, pen: &PenBounds) {
    let start = find_start(grid, width, height, pen);
    if start.is_none() {
        return;
    }
    let mut reachable = flood(grid, width, height, pen, start.unwrap());

    let mut iterations = 0;
    while has_unreachable(grid, width, height, pen, &reachable) && iterations < width * height {
        let mut carved = false;
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let pos = Pos { x, y };
                if grid[y][x] != Tile::Wall {
                    continue;
                }
                if is_pen_wall(pos, pen) {
                    continue;
                }
                if grid[y][x] == Tile::Gate {
                    continue;
                }
                let mut has_reach = false;
                let mut has_unreach = false;
                for (dx, dy) in [(0isize, -1isize), (0, 1), (-1, 0), (1, 0)] {
                    let nx = (x as isize + dx) as usize;
                    let ny = (y as isize + dy) as usize;
                    let npos = Pos { x: nx, y: ny };
                    if !is_walkable_for_player(grid, width, height, pen, npos) {
                        continue;
                    }
                    if reachable[ny][nx] {
                        has_reach = true;
                    } else {
                        has_unreach = true;
                    }
                }
                if has_reach && has_unreach {
                    grid[y][x] = Tile::Empty;
                    carved = true;
                    break;
                }
            }
            if carved {
                break;
            }
        }

        if !carved {
            break;
        }
        reachable = flood(grid, width, height, pen, start.unwrap());
        iterations += 1;
    }
}

fn find_start(
    grid: &[Vec<Tile>],
    width: usize,
    height: usize,
    pen: &PenBounds,
) -> Option<Pos> {
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let pos = Pos { x, y };
            if is_walkable_for_player(grid, width, height, pen, pos) {
                return Some(pos);
            }
        }
    }
    None
}

fn is_walkable_for_player(
    grid: &[Vec<Tile>],
    _width: usize,
    _height: usize,
    pen: &PenBounds,
    pos: Pos,
) -> bool {
    if is_in_pen_bounds(pos, pen) {
        return false;
    }
    match grid[pos.y][pos.x] {
        Tile::Wall | Tile::Gate => false,
        _ => true,
    }
}

fn is_in_pen_bounds(pos: Pos, pen: &PenBounds) -> bool {
    pos.x >= pen.x0 && pos.x <= pen.x1 && pos.y >= pen.y0 && pos.y <= pen.y1
}

fn flood(
    grid: &[Vec<Tile>],
    width: usize,
    height: usize,
    pen: &PenBounds,
    start: Pos,
) -> Vec<Vec<bool>> {
    let mut seen = vec![vec![false; width]; height];
    let mut q = VecDeque::new();
    seen[start.y][start.x] = true;
    q.push_back(start);
    while let Some(pos) = q.pop_front() {
        for (dx, dy) in [(0isize, -1isize), (0, 1), (-1, 0), (1, 0)] {
            let nx = pos.x as isize + dx;
            let ny = pos.y as isize + dy;
            if nx <= 0 || ny <= 0 || nx >= (width - 1) as isize || ny >= (height - 1) as isize
            {
                continue;
            }
            let nx = nx as usize;
            let ny = ny as usize;
            let npos = Pos { x: nx, y: ny };
            if seen[ny][nx] {
                continue;
            }
            if !is_walkable_for_player(grid, width, height, pen, npos) {
                continue;
            }
            seen[ny][nx] = true;
            q.push_back(npos);
        }
    }
    seen
}

fn has_unreachable(
    grid: &[Vec<Tile>],
    width: usize,
    height: usize,
    pen: &PenBounds,
    reachable: &[Vec<bool>],
) -> bool {
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let pos = Pos { x, y };
            if is_walkable_for_player(grid, width, height, pen, pos) && !reachable[y][x] {
                return true;
            }
        }
    }
    false
}

fn ghost_next_dir_pen(
    pos: Pos,
    grid: &[Vec<Tile>],
    width: usize,
    height: usize,
    pen: &PenBounds,
    rng: &mut impl Rng,
) -> Option<Dir> {
    let mut options = Vec::new();
    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        if !can_move_ghost(grid, width, height, pos, dir, false) {
            continue;
        }
        let next = step(pos, dir);
        if in_pen_interior(next, pen) {
            options.push(dir);
        }
    }
    options.choose(rng).copied()
}

fn random_bonus_spawn(game: &Game, rng: &mut impl Rng) -> Option<Pos> {
    let mut candidates = Vec::new();
    for y in 1..game.height - 1 {
        for x in 1..game.width - 1 {
            if game.grid[y][x] == Tile::Empty {
                let pos = Pos { x, y };
                if is_in_pen(pos, game.width, game.height) {
                    continue;
                }
                if game.player == pos {
                    continue;
                }
                if game.ghosts.iter().any(|g| *g == pos) {
                    continue;
                }
                candidates.push(pos);
            }
        }
    }
    candidates.choose(rng).copied()
}

fn braid_maze(grid: &mut [Vec<Tile>], cells_w: usize, cells_h: usize, rng: &mut impl Rng) {
    for cy in 0..cells_h {
        for cx in 0..cells_w {
            let open = cell_open_neighbors(grid, cx, cy, cells_w, cells_h);
            let closed = cell_closed_neighbors(grid, cx, cy, cells_w, cells_h);

            if open.len() == 1 && !closed.is_empty() && rng.gen::<f32>() < BRAID_CHANCE {
                let dir = *closed.choose(rng).unwrap();
                carve_between_dir(grid, cx, cy, dir);
            } else if !closed.is_empty() && rng.gen::<f32>() < EXTRA_OPENINGS {
                let dir = *closed.choose(rng).unwrap();
                carve_between_dir(grid, cx, cy, dir);
            }
        }
    }
}

fn carve_between_dir(grid: &mut [Vec<Tile>], cx: usize, cy: usize, dir: Dir) {
    let (dx, dy) = dir.delta();
    let nx = (cx as isize + dx) as usize;
    let ny = (cy as isize + dy) as usize;
    carve_between(grid, cx, cy, nx, ny);
    carve_cell(grid, nx, ny);
}

fn cell_open_neighbors(
    grid: &[Vec<Tile>],
    cx: usize,
    cy: usize,
    cells_w: usize,
    cells_h: usize,
) -> Vec<Dir> {
    let mut open = Vec::new();
    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        let (dx, dy) = dir.delta();
        let nx = cx as isize + dx;
        let ny = cy as isize + dy;
        if nx < 0 || ny < 0 {
            continue;
        }
        let nx = nx as usize;
        let ny = ny as usize;
        if nx >= cells_w || ny >= cells_h {
            continue;
        }
        if is_open_between(grid, cx, cy, nx, ny) {
            open.push(dir);
        }
    }
    open
}

fn cell_closed_neighbors(
    grid: &[Vec<Tile>],
    cx: usize,
    cy: usize,
    cells_w: usize,
    cells_h: usize,
) -> Vec<Dir> {
    let mut closed = Vec::new();
    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        let (dx, dy) = dir.delta();
        let nx = cx as isize + dx;
        let ny = cy as isize + dy;
        if nx < 0 || ny < 0 {
            continue;
        }
        let nx = nx as usize;
        let ny = ny as usize;
        if nx >= cells_w || ny >= cells_h {
            continue;
        }
        if !is_open_between(grid, cx, cy, nx, ny) {
            closed.push(dir);
        }
    }
    closed
}

fn is_open_between(grid: &[Vec<Tile>], cx: usize, cy: usize, nx: usize, ny: usize) -> bool {
    let gx = cx * 2 + 1;
    let gy = cy * 2 + 1;
    let ngx = nx * 2 + 1;
    let ngy = ny * 2 + 1;
    let wall_x = (gx + ngx) / 2;
    let wall_y = (gy + ngy) / 2;
    grid[wall_y][wall_x] != Tile::Wall
}
