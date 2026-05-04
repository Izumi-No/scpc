pub struct TimerWheel {
    slots: Vec<Vec<usize>>,
    current: usize,
}

impl TimerWheel {
    pub fn new(size: usize) -> Self {
        Self {
            slots: vec![Vec::new(); size],
            current: 0,
        }
    }

    /// Create a timer wheel with per-slot capacity pre-reserved. This helps benches avoid
    /// noisy allocations when adding a large number of timers distributed across slots.
    pub fn with_slot_capacity(size: usize, slot_capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(size);
        for _ in 0..size {
            let mut v = Vec::new();
            v.reserve(slot_capacity);
            slots.push(v);
        }
        Self { slots, current: 0 }
    }

    pub fn tick(&mut self) -> Vec<usize> {
        let expired = std::mem::take(&mut self.slots[self.current]);
        self.current = (self.current + 1) % self.slots.len();
        expired
    }

    pub fn add(&mut self, conn_id: usize, ticks_ahead: usize) {
        let target = (self.current + ticks_ahead) % self.slots.len();
        self.slots[target].push(conn_id);
    }
}

//profiling and testing
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_timer_wheel() {
        let mut wheel = TimerWheel::new(4);
        wheel.add(1, 1);
        wheel.add(2, 2);
        wheel.add(3, 3);
    }
}
