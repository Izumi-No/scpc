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
