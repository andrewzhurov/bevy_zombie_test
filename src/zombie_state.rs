use bevy_life::CellState;
#[cfg(feature = "auto-coloring")]
use bevy::prelude::Color;
use bevy::prelude::Component;
use std::ops::{Deref, DerefMut};

/// Direction deltas indexed clockwise starting from North
pub const DIRECTION_DELTAS: [(i32, i32); 9] = [
    /* 0: North      */ ( 0, -1),
    /* 1: Northeast  */ ( 1, -1),
    /* 2: East       */ ( 1,  0),
    /* 3: Southeast  */ ( 1,  1),
    /* 4: South      */ ( 0,  1),
    /* 5: Southwest  */ (-1,  1),
    /* 6: West       */ (-1,  0),
    /* 7: Northwest  */ (-1, -1),
    /* 8: No direction */ ( 0,  0),
];


#[derive(Debug, Clone, Default, Eq, PartialEq, Component)]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
pub struct ZombieState(pub Vec<i32>);

/*
    Each cell represents a piece of terrain with the following values:
    - 0: X coordinate (immutable, from terrain generation)
    - 1: Y coordinate (immutable, from terrain generation)
    - 2: Altitude (immutable, from terrain generation)
    - 3: Temperature (immutable, from terrain generation)
    - 4: Status (0: Empty, 1: Zombie, 2: Human)
    - 5: Population
    - 6: Direction (Where they will either attack or reinforce on the next turn) (range 0-7), use own coordinate and neighbor coordinate to determine if incoming
        0: North        (0, -1)
        1: Northeast    (1, -1)
        2: East         (1, 0)
        3: Southeast    (1, 1)
        4: South        (0, 1)
        5: Southwest    (-1, 1)
        6: West         (-1, 0)
        7: Northwest    (-1, -1)
        8: No direction (no attack or reinforcement planned)
    - 7: Human smell (0-100, 0 means no smell, 100 means very strong smell)
    - 8: Zombie smell (0-100, 0 means no smell, 100 means very strong smell)

    7-8 are used for AI decision making They are averaged over neighbors, then incremented by the population of the cell.
    This way zombies and humans can detect each other from several cells away since smells propagate.

*/

impl CellState for ZombieState {
    fn new_cell_state<'a>(&self, neighbor_cells: impl Iterator<Item = &'a Self>) -> Self {
        // Apply attack or reinforce from neighbors first, then update population, finally update intentions
        let neighbors: Vec<&Self> = neighbor_cells.collect();

        // Next, look at the Direction value of all neighbors to see if any are sending zombies/humans our way.
        let mut incoming_humans = 0;
        let mut incoming_zombies = 0;
        for neighbor in &neighbors {
            // Check neighbor's direction to see if what they are sending is coming our way
            // Find the DIRECTION_DELTA that matches the difference between our coordinates and the neighbor's coordinates
            let delta = (
                neighbor.0[0] - self.0[0],
                neighbor.0[1] - self.0[1],
            );
            if neighbor.0[6] == delta_to_direction(delta).unwrap() {
                // If the neighbor is sending something our way, increment the appropriate counter
                if neighbor.0[4] == 1 { // Zombie
                    incoming_zombies += neighbor.0[5];
                } else if neighbor.0[4] == 2 { // Human
                    incoming_humans += neighbor.0[5];
                }
            }
        }

        // Now, update our own state based on incoming zombies and humans
        // Count how many zombies and humans we have (including ourselves). Give advantage to whichever holds this cell.
        let total_humans = incoming_humans + if self.0[4] == 2 && self.0[6] != 8 { self.0[5] } else { 0 }; // Our own population only counts if they didn't move away on the last turn!
        let total_zombies = incoming_zombies + if self.0[4] == 1 && self.0[6] != 8 { self.0[5] } else { 0 };
        // Defender's advantage: offense must be 3x defender's population to take the cell
        let mut new_state = self.0.clone();
        if self.0[4] == 0 {
            // If empty, cell goes to the larger population, subtract the population of the smaller one
            if total_humans > total_zombies {
                new_state[4] = 2; // Human
                new_state[5] = total_humans - total_zombies; // New population
            } else if total_zombies > total_humans {
                new_state[4] = 1; // Zombie
                new_state[5] = total_zombies - total_humans; // New population
            } else {
                new_state[4] = 0; // Still empty
                new_state[5] = 0; // No population
            }
        } else if self.0[4] == 1 {
            // If zombie, check if we can hold the cell
            if total_zombies < total_humans {
                new_state[4] = 2; // Cell goes to humans
                new_state[5] = total_humans - total_zombies; // New population
            } else {
                new_state[5] = total_zombies - total_humans; // New population
            }
            new_state[5] += total_humans / 3; // Add 1/3 of humans to zombies to simulate the zombie infection spread
            //println!("Added {} zombies to cell at ({}, {})", total_humans / 3, self.0[0], self.0[1]);
        } else if self.0[4] == 2 {
            // If human, check if we can hold the cell
            if total_humans < total_zombies / 3 {// 
                new_state[4] = 1; // Cell goes to zombies
                new_state[5] = total_zombies / 3 - total_humans; // New population
            } else {
                new_state[5] = total_humans - total_zombies / 3; // New population
            }
            new_state[5] += total_humans / 10; // Add 1/10 of humans to humans to simulate human reproduction
            //println!("Added {} humans to cell at ({}, {})", total_humans / 10, self.0[0], self.0[1]);
        }
        // Update smell and noise. Set to average of neighbors, then add 1 for each population (human or zombie) in the cell.
        new_state[7] = neighbors.iter()
            .map(|n| n.0[7])
            .sum::<i32>() / neighbors.len() as i32 + if self.0[4] == 2 { self.0[5] } else { 0 };
        new_state[8] = neighbors.iter()
            .map(|n| n.0[8])
            .sum::<i32>() / neighbors.len() as i32 + if self.0[4] == 1 { self.0[5] } else { 0 };

        // Finally, look at the smells of neighbors to determine our next direction
        let mut direction = 8; // Default to no direction
        // If we're zombies, mindlessly attack the strongest smell of humans.
        // If we're humans, hunker down unless we detect a zombie population significantly smaller than ours.
        if new_state[4] == 1 { // Zombie
            let mut max_smell = 0;
            for neighbor in &neighbors {
                if neighbor.0[7] > max_smell {
                    max_smell = neighbor.0[7];
                    direction = neighbor.0[6]; // Take the direction of the strongest human smell
                }
            }
        } else if new_state[4] == 2 { // Human
            let mut min_smell = 0;
            let mut zombie_population = 0;
            let mut neighbor_state = 0;
            for neighbor in &neighbors {
                if neighbor.0[8] < min_smell {
                    min_smell = neighbor.0[8];
                    direction = neighbor.0[6]; // Take the direction of the smallest zombie smell
                    zombie_population = if neighbor.0[4] == 1 {neighbor.0[5]} else {0}; // Get the population of the smallest zombie smell
                    neighbor_state = neighbor.0[4]; // Get the state of the neighbor
                }
            }
            // Check if smallest zombie smell is from a zombie cell, if so check population. If population is less than 1/3 of ours, attack.
            // If not a zombie cell, check if zombie smell is less than own cell - if so it's probably a safer cell
            if direction != 8 && new_state[5] > 0 && zombie_population < new_state[5] / 3 {
                // If we have a strong enough population, attack
                direction = delta_to_direction(DIRECTION_DELTAS[direction as usize]).unwrap_or(8);
            } else if direction != 8 && new_state[5] > 0 && neighbor_state != 1 && min_smell < new_state[8] {
                // If the smallest zombie smell is not from a zombie cell, and the smell is less than our own, it's probably a safer cell than ours, the population should move there.
                direction = delta_to_direction(DIRECTION_DELTAS[direction as usize]).unwrap_or(8);
            }
        }
        new_state[6] = direction; // Update our direction based on smells

        // Check if population is zero, if so set state to empty and direction to no direction (8)
        if new_state[5] <= 0 {
            new_state[4] = 0; // Set to empty
            new_state[6] = 8; // No direction
        }

        if self.0[4] != new_state[4] {
            //println!("Cell at ({}, {}) changed from {:?} to {:?}", self.0[0], self.0[1], self.0[4], new_state[4]);
        }

        Self(new_state)
    }

    #[cfg(feature = "auto-coloring")]
    fn color(&self) -> Option<bevy::prelude::Color> {
        match self.0[4] {
            0 => Some(Color::BLACK), // Empty
            1 => Some(Color::srgba(0., 1., 0., 1.)), // Zombie Green
            2 => Some(Color::srgba(0., 0., 1., 1.)), // Human Blue
            _ => None, // Invalid state
        }
    }
}

impl Deref for ZombieState {
    type Target = Vec<i32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ZombieState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<i32>> for ZombieState {
    fn from(vec: Vec<i32>) -> Self {
        ZombieState(vec)
    }
}

pub fn delta_to_direction(delta: (i32, i32)) -> Option<i32> {
    match delta {
        (0, -1) => Some(0),  // North
        (1, -1) => Some(1),  // Northeast
        (1, 0)  => Some(2),  // East
        (1, 1)  => Some(3),  // Southeast
        (0, 1)  => Some(4),  // South
        (-1, 1) => Some(5),  // Southwest
        (-1, 0) => Some(6),  // West
        (-1, -1) => Some(7), // Northwest
        (0, 0)  => Some(8),  // No direction
        _ => None,
    }// Faster than a loop in 87% of cases, and more readable
}
