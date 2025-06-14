use bevy::prelude::warn;
use bevy::{audio::CpalSample, math::IVec2, prelude::Component};
use bevy_life::CellState;
use std::cmp::Ordering;

#[derive(Debug, Clone, Default, Eq, PartialEq, Component)]
pub enum Status {
    #[default]
    Empty,
    Zombie,
    Human,
}

impl Status {
    #[inline]
    fn is_empty(&self) -> bool {
        self == &Self::Empty
    }

    #[inline]
    fn is_human(&self) -> bool {
        self == &Self::Human
    }

    #[inline]
    fn is_zombie(&self) -> bool {
        self == &Self::Zombie
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Component)]
pub struct ZombieState {
    pub xy: IVec2,        // (immutable, from terrain generation)
    pub altitude: i32,    // (immutable, from terrain generation)
    pub temperature: i32, // (immutable, from terrain generation)
    pub status: Status,
    pub population: i32,
    pub direction: i8, // (Where they will either attack or reinforce on the next turn) (range 0-7), use own coordinate and neighbor coordinate to determine if incoming
    pub smell_human: i32, // Human smell (0-100, 0 means no smell, 100 means very strong smell)
    pub smell_zombie: i32, // Zombie smell (0-100, 0 means no smell, 100 means very strong smell)
}

impl CellState for ZombieState {
    fn new_cell_state<'a>(&self, neighbor_cells: impl Iterator<Item = &'a Self>) -> Self {
        // println!("-----------------------------------");
        // println!("Cell: {self:?}");

        // Apply attack or reinforce from neighbors first, then update population, finally update intentions
        let neighbors: Vec<&Self> = neighbor_cells.collect();
        // println!("neighbors: {neighbors:?}");

        // Next, look at the Direction value of all neighbors to see if any are sending zombies/humans our way.
        let mut incoming_humans = 0;
        let mut incoming_zombies = 0;
        for neighbor in &neighbors {
            // Check neighbor's direction to see if what they are sending is coming our way
            // Find the DIRECTION_DELTA that matches the difference between our coordinates and the neighbor's coordinates
            let delta = self.xy - neighbor.xy;
            if neighbor.direction == delta_to_direction(delta).unwrap() {
                // If the neighbor is sending something our way, increment the appropriate counter
                if neighbor.status.is_zombie() {
                    incoming_zombies += neighbor.population;
                } else if neighbor.status.is_human() {
                    incoming_humans += neighbor.population;
                }
            }
        }

        // println!("incoming_humans: {incoming_humans}");
        // println!("incoming_zombies: {incoming_zombies}");

        // Now, update our own state based on incoming zombies and humans
        // Count how many zombies and humans we have (including ourselves). Give advantage to whichever holds this cell.
        let total_humans = incoming_humans
            + if self.status.is_human() && self.direction == 8 {
                self.population
            } else {
                0 // Our own population only counts if they didn't move away on the last turn!
            };

        let total_zombies = incoming_zombies
            + if self.status.is_zombie() && self.direction == 8 {
                self.population
            } else {
                0
            };

        // println!("total_humans: {total_humans}");
        // println!("total_zombies: {total_zombies}");

        let mut new_state = self.clone();

        let humans_cmp_zombies = total_humans.cmp(&total_zombies);

        // Fight!
        match new_state.status {
            Status::Empty => {
                // If empty, cell goes to the larger population, subtract the population of the smaller one
                match humans_cmp_zombies {
                    Ordering::Greater => {
                        new_state.status = Status::Human;
                        new_state.population = total_humans - total_zombies;
                    }
                    Ordering::Less => {
                        new_state.status = Status::Zombie;
                        new_state.population = total_zombies - total_humans;
                    }
                    Ordering::Equal => {
                        new_state.status = Status::Empty;
                        new_state.population = 0;
                    }
                }
            }
            Status::Zombie => {
                // Check whether humans can take the cell
                match humans_cmp_zombies {
                    Ordering::Greater => {
                        new_state.status = Status::Human;
                        new_state.population = total_humans - total_zombies;
                    }
                    Ordering::Less => {
                        // Add 1/3 of humans to zombies to simulate the zombie infection spread
                        new_state.population = total_zombies - total_humans + total_humans / 3;
                    }
                    Ordering::Equal => {
                        new_state.status = Status::Empty;
                        new_state.population = 0;
                    }
                }
            }
            Status::Human => {
                // Check if humans can hold the cell
                // Human's have holder's advantage of 1 to 3, i.e., one human can take out 1 zombie.
                match total_humans.cmp(&(total_zombies / 3)) {
                    Ordering::Greater => {
                        new_state.population = total_humans - total_zombies / 3;
                        // TODO "turned humans during combat"
                    }
                    Ordering::Less => {
                        new_state.status = Status::Zombie;
                        new_state.population = total_zombies - total_humans * 3 + total_humans / 3;
                    }
                    Ordering::Equal => {
                        new_state.status = Status::Empty;
                        new_state.population = 0; // Well, there should actually be some turned humans left after this fight
                    }
                }
            }
        }

        // Check if population is zero, if so set its state to empty (just double-checking)
        if new_state.population == 0 {
            new_state.status = Status::Empty;
        }

        if new_state.population < 0 {
            warn!(
                "Cell's population is negative!\n Cell: {new_state:?}\n Neighbors: {neighbors:?}"
            );
        }

        // println!("Battle ended, new_state: {new_state:?}");

        if new_state.status.is_human() {
            new_state.population = new_state.population.mul_amp(1.01); // Simulate birth rate, 1%
                                                                       // println!("Human population grew: {}", new_state.population);
        }

        // Update smell and noise. Set to average of neighbors, then add 1 for each population (human or zombie) in the cell.
        new_state.smell_human = neighbors.iter().map(|n| n.smell_human).sum::<i32>()
            / neighbors.len() as i32
            + if self.status.is_human() {
                self.population
            } else {
                0
            };
        new_state.smell_zombie = neighbors.iter().map(|n| n.smell_zombie).sum::<i32>()
            / neighbors.len() as i32
            + if self.status.is_zombie() {
                self.population
            } else {
                0
            };

        // Finally, look at the smells of neighbors to determine our next direction
        new_state.direction = 8; // Default to no direction

        // If we're zombies, mindlessly follow the strongest smell of humans.
        // If we're humans, hunker down unless we detect a zombie population significantly smaller than ours.
        match new_state.status {
            Status::Zombie => {
                let preferred_neighbor = neighbors
                    .iter()
                    .max_by(|n1, n2| {
                        match n1.smell_human.cmp(&n2.smell_human) {
                            Ordering::Equal => {
                                match n1.temperature.cmp(&n2.temperature) {
                                    Ordering::Equal => {
                                        n1.altitude.cmp(&n2.altitude).reverse() // zombies prefer lower places
                                    }
                                    non_eq => non_eq.reverse(), // zombies preffer cold places
                                }
                            }
                            non_eq => non_eq,
                        }
                    })
                    .unwrap();

                new_state.direction = delta_to_direction(preferred_neighbor.xy - self.xy).unwrap();
            }
            Status::Human => {
                let preferred_neighbor = neighbors
                    .iter()
                    .max_by(|n1, n2| match n1.smell_zombie.cmp(&n2.smell_zombie) {
                        Ordering::Equal => {
                            match n1.temperature.cmp(&n2.temperature) {
                                Ordering::Equal => {
                                    n1.altitude.cmp(&n2.altitude) // people prefer higher places, it's a zombie apoc, high is safer!
                                }
                                non_eq => non_eq, // people prefer warmer places
                            }
                        }
                        non_eq => non_eq.reverse(), // people prefer places with less zombie smell, this is ImPoRtAnT! (for living to see another day)
                    })
                    .unwrap();

                let preferred_neighbor_zombie_population = if preferred_neighbor.status.is_zombie()
                {
                    preferred_neighbor.population
                } else {
                    0
                };

                // Move to min zombie smell cell if either
                // We outnumber zombies > 3:1 - attack!
                // It has smaller than ours zombie smell - it's probably a safer cell than ours.
                if (new_state.population / 3 > preferred_neighbor_zombie_population)
                    || (!preferred_neighbor.status.is_zombie()
                        && preferred_neighbor.smell_zombie < new_state.smell_zombie)
                {
                    new_state.direction =
                        delta_to_direction(preferred_neighbor.xy - self.xy).unwrap();
                }
            }
            _ => {}
        }

        if self.status != new_state.status {
            // println!(
            //     "Cell at {:?} changed from {:?} to {:?}",
            //     self.xy, self.status, new_state.status
            // );
        }

        // println!("new_state: {new_state:?}");

        new_state
    }
}

impl From<Vec<i32>> for ZombieState {
    fn from(vec: Vec<i32>) -> Self {
        ZombieState {
            xy: IVec2::new(vec[0], vec[1]),
            altitude: vec[2],
            temperature: vec[3],
            status: match vec[4] {
                1 => Status::Zombie,
                2 => Status::Human,
                _ => Status::Empty,
            },
            population: vec[5],
            direction: vec[6] as i8,
            smell_human: vec[7],
            smell_zombie: vec[8],
        }
    }
}

pub fn delta_to_direction(delta: IVec2) -> Option<i8> {
    match (delta.x, delta.y) {
        (0, -1) => Some(0),  // North
        (1, -1) => Some(1),  // Northeast
        (1, 0) => Some(2),   // East
        (1, 1) => Some(3),   // Southeast
        (0, 1) => Some(4),   // South
        (-1, 1) => Some(5),  // Southwest
        (-1, 0) => Some(6),  // West
        (-1, -1) => Some(7), // Northwest
        (0, 0) => Some(8),   // No direction
        _ => None,
    } // Faster than a loop in 87% of cases, and more readable
}
