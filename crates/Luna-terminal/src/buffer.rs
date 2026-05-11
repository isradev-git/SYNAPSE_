use crate::grid::CharCell;

pub struct ScrollbackBuffer {
    lines: Vec<Option<Vec<CharCell>>>,
    capacity: usize,
    head: usize,
    count: usize,
}

impl ScrollbackBuffer {
    pub fn new(capacity: usize) -> Self {
        let mut lines = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            lines.push(None);
        }
        Self {
            lines,
            capacity,
            head: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, line: Vec<CharCell>) {
        if self.capacity == 0 {
            return;
        }
        self.lines[self.head] = Some(line);
        self.head = (self.head + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    pub fn get_line(&self, index: usize) -> &[CharCell] {
        if index >= self.count {
            return &[];
        }
        let start = if self.count < self.capacity {
            0
        } else {
            self.head
        };
        let actual = (start + index) % self.capacity;
        match &self.lines[actual] {
            Some(line) => line.as_slice(),
            None => &[],
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_get() {
        let mut buf = ScrollbackBuffer::new(5);
        let line1: Vec<CharCell> = vec![CharCell {
            c: 'A',
            ..CharCell::default()
        }];
        buf.push(line1);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.get_line(0)[0].c, 'A');
    }

    #[test]
    fn test_circular_overflow() {
        let mut buf = ScrollbackBuffer::new(3);
        for i in 0u8..5 {
            let line: Vec<CharCell> = vec![CharCell {
                c: (b'0' + i) as char,
                ..CharCell::default()
            }];
            buf.push(line);
        }
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.get_line(0)[0].c, '2');
        assert_eq!(buf.get_line(1)[0].c, '3');
        assert_eq!(buf.get_line(2)[0].c, '4');
    }

    #[test]
    fn test_large_buffer() {
        let mut buf = ScrollbackBuffer::new(100);
        for _ in 0..200u32 {
            let line: Vec<CharCell> = vec![CharCell {
                c: 'X',
                ..CharCell::default()
            }];
            buf.push(line);
        }
        assert_eq!(buf.len(), 100);
    }
}
