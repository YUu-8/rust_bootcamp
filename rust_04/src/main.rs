use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    cursor,
    style::{self, Color, Stylize},
    terminal::{self, Clear, ClearType},
    ExecutableCommand, QueueableCommand,
};
use rand::Rng;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

// --- CLI  ---

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(value_name = "FILE")]
    map_file: Option<PathBuf>,

    #[arg(short, long)]
    generate: Option<String>,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(short, long)]
    visualize: bool,

    #[arg(short, long)]
    both: bool,

    #[arg(short, long)]
    animate: bool,
}

// --- Grid ---

#[derive(Clone)]
struct Grid {
    width: usize,
    height: usize,
    cells: Vec<u8>,
}

impl Grid {
    fn get(&self, x: usize, y: usize) -> Option<u8> {
        if x < self.width && y < self.height {
            Some(self.cells[y * self.width + x])
        } else {
            None
        }
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn from_str(input: &str) -> Result<Self> {
        let values: Vec<u8> = input
            .split_whitespace()
            .map(|s| u8::from_str_radix(s, 16).context("Invalid hex"))
            .collect::<Result<_, _>>()?;

        let lines: Vec<&str> = input.trim().lines().collect();
        let height = lines.len();
        if height == 0 { anyhow::bail!("Empty grid"); }
        let width = values.len() / height;

        Ok(Self { width, height, cells: values })
    }

    fn generate(width: usize, height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut cells = vec![0u8; width * height];
        for i in 0..cells.len() {
            cells[i] = rng.gen();
        }
        cells[0] = 0x00;
        cells[width * height - 1] = 0xFF;
        Self { width, height, cells }
    }

    fn save(&self, path: &PathBuf) -> Result<()> {
        let mut file = File::create(path)?;
        for y in 0..self.height {
            for x in 0..self.width {
                write!(file, "{:02X} ", self.cells[self.index(x, y)])?;
            }
            writeln!(file)?;
        }
        Ok(())
    }
}

// --- Dijkstra ---

#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: u32,
    position: (usize, usize),
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost).then_with(|| self.position.cmp(&other.position))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn solve_dijkstra(grid: &Grid, animate: bool) -> (Option<Vec<(usize, usize)>>, u32) {
    let start = (0, 0);
    let end = (grid.width - 1, grid.height - 1);

    let mut dists: HashMap<(usize, usize), u32> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();

    dists.insert(start, 0);
    heap.push(State { cost: 0, position: start });

    let mut stdout = io::stdout();

    if animate {
        stdout.execute(Clear(ClearType::All)).unwrap();
    }

    let mut step = 0;

    while let Some(State { cost, position }) = heap.pop() {
        let (cx, cy) = position;

        if animate {
             step += 1;
             stdout.queue(cursor::MoveTo(0, 0)).unwrap();
             println!("Step {}: Exploring ({},{}) - cost: {}", step, cx, cy, cost);
  
             let mut current_path_set = HashSet::new();
             let mut temp = position;
             while let Some(&prev) = came_from.get(&temp) {
                 current_path_set.insert(prev);
                 temp = prev;
             }

             draw_grid_animation(grid, position, &current_path_set);
             stdout.flush().unwrap();
             thread::sleep(Duration::from_millis(100)); 
        }

        if position == end {

            let mut path = vec![end];
            let mut curr = end;
            while let Some(&prev) = came_from.get(&curr) {
                path.push(prev);
                curr = prev;
            }
            path.reverse();
            if animate {
                 step += 1;
                 stdout.queue(cursor::MoveTo(0, 0)).unwrap();
                 println!("Step {}: Path found!                     ", step); 
                
                 let mut final_set = HashSet::new();
                 for &p in &path {
                     final_set.insert(p);
                 }
                 draw_grid_animation(grid, (usize::MAX, usize::MAX), &final_set); 
                 stdout.flush().unwrap();
                 println!(); 
            }

            return (Some(path), cost + grid.get(end.0, end.1).unwrap() as u32);
        }

        if cost > *dists.get(&position).unwrap_or(&u32::MAX) {
            continue;
        }

        let moves = [(0, -1), (0, 1), (-1, 0), (1, 0)]; 

        for (dx, dy) in moves {
            let nx = cx as isize + dx;
            let ny = cy as isize + dy;

            if nx >= 0 && nx < grid.width as isize && ny >= 0 && ny < grid.height as isize {
                let next_pos = (nx as usize, ny as usize);
                let weight = grid.get(next_pos.0, next_pos.1).unwrap() as u32;
                let next_cost = cost + weight;

                if next_cost < *dists.get(&next_pos).unwrap_or(&u32::MAX) {
                    heap.push(State { cost: next_cost, position: next_pos });
                    dists.insert(next_pos, next_cost);
                    came_from.insert(next_pos, position);
                }
            }
        }
    }

    (None, 0)
}

// --- drawing ---

fn get_color(val: u8) -> Color {
    if val < 42 { Color::Red }
    else if val < 84 { Color::Yellow }
    else if val < 126 { Color::Green }
    else if val < 168 { Color::Cyan }
    else if val < 210 { Color::Blue }
    else { Color::Magenta }
}

fn draw_grid_colored(grid: &Grid, path: Option<&Vec<(usize, usize)>>, is_max_path: bool) {
    let path_set: Vec<(usize, usize)> = path.cloned().unwrap_or_default();
    
    println!("{}", "=".repeat(grid.width * 3));

    for y in 0..grid.height {
        for x in 0..grid.width {
            let val = grid.get(x, y).unwrap();
            let is_in_path = path_set.contains(&(x, y));
            
            let text = format!("{:02X}", val);
            
            if is_in_path {
                if is_max_path {
                     print!("{} ", text.with(Color::Red).bold());
                } else {
                     print!("{} ", text.with(Color::White).bold());
                }
            } else {
                print!("{} ", text.with(get_color(val)));
            }
        }
        println!();
    }
    println!();
}


fn draw_grid_animation(grid: &Grid, current: (usize, usize), path_trace: &HashSet<(usize, usize)>) {
    for y in 0..grid.height {
        for x in 0..grid.width {
            if (x, y) == current {
                print!("[*]"); 
            } else if path_trace.contains(&(x, y)) {
                print!("[✓]"); 
            } else {
                print!("[ ]"); 
            }
        }
        println!();
    }
}

// --- main ---

fn main() -> Result<()> {
    let args = Args::parse();
    let mut grid: Option<Grid> = None;

    if let Some(gen_str) = args.generate {
        let parts: Vec<&str> = gen_str.split('x').collect();
        if parts.len() != 2 {
            anyhow::bail!("Format incorrect. Use WxH (e.g., 12x8)");
        }
        let w: usize = parts[0].parse()?;
        let h: usize = parts[1].parse()?;
        let g = Grid::generate(w, h);
        println!("Generating {}x{} hexadecimal grid...", w, h);
        
        if let Some(out_path) = &args.output {
            g.save(out_path)?;
            println!("Map saved to: {:?}", out_path);
        }
        
        println!("Generated Map:");
        for y in 0..g.height {
            for x in 0..g.width {
                print!("{:02X} ", g.get(x,y).unwrap());
            }
            println!();
        }
        println!();
        grid = Some(g);
    } else if let Some(path) = args.map_file {
        println!("Analyzing hexadecimal grid from {:?}...", path);
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        grid = Some(Grid::from_str(&contents)?);
    }

    if grid.is_none() {
        if std::env::args().len() == 1 {
            use clap::CommandFactory;
            Args::command().print_help()?;
            return Ok(());
        }
        return Ok(());
    }

    let grid = grid.unwrap();

    if args.visualize && !args.both && !args.animate {
        println!("HEXADECIMAL GRID (rainbow gradient):");
        draw_grid_colored(&grid, None, false);
        return Ok(());
    }

    if args.animate {
        println!("Searching for minimum cost path...");
        thread::sleep(Duration::from_millis(500));
        solve_dijkstra(&grid, true);
    } else {
        println!("Finding optimal paths...");
        let (path, cost) = solve_dijkstra(&grid, false);
        
        if let Some(p) = &path {
            println!("MINIMUM COST PATH:");
            println!("Total cost: 0x{:X} ({} decimal)", cost, cost);
            println!("Path length: {} steps", p.len());
            
            let path_str: Vec<String> = p.iter().map(|(x,y)| format!("({},{})", x, y)).collect();
            if path_str.len() > 10 {
                 println!("Path: {} ... -> {}", path_str[0..5].join("->"), path_str.last().unwrap());
            } else {
                 println!("Path: {}", path_str.join("->"));
            }

            println!("\nStep-by-step costs:");
            let mut accum = 0;
            for (i, &(x, y)) in p.iter().enumerate() {
                let val = grid.get(x, y).unwrap() as u32;
                accum += val;
                if i < 5 || i > p.len() - 5 { 
                    println!("-> 0x{:02X} ({}, {}) +{}", val, x, y, accum);
                }
            }
            println!("Total: 0x{:X} ({})", accum, accum);

            println!("\nMINIMUM COST PATH (shown in WHITE):");
            draw_grid_colored(&grid, Some(p), false);
        }

        if args.both {
             println!("\nMAXIMUM COST PATH (shown in RED):");
             draw_grid_colored(&grid, path.as_ref(), true); 
        }
    }

    Ok(())
}