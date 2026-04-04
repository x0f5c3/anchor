/// Trait for output buffers that can accept encoded data.
///
/// Message builders accept an argumenet of this type and will output their data in to the buffer.
///
/// The buffer must support simple seeking to a previously retrieved position. This is used when
/// calculating checksums.
pub trait OutputBuffer {
    /// The cursor type
    type Cursor: Copy;
    /// Append bytes to the buffer
    fn output(&mut self, buf: &[u8]);
    /// Retrieve the cursor representing the current write position
    fn cur_position(&self) -> Self::Cursor;
    /// Replace the byte at the cursor position with a new value
    fn update(&mut self, cursor: Self::Cursor, value: u8);
    /// Retrieve a reference to all data pushed after the cursor
    fn data_since(&self, cursor: Self::Cursor) -> &[u8];
    /// Roll back write position to a previously saved cursor,
    /// discarding all bytes written after that point.
    fn rollback(&mut self, cursor: Self::Cursor);
}

/// A scratch pad based `OutputBuffer`.
///
/// Uses a statically sized inlined buffer. For serializing multiple messages in a row, the buffer
/// can be reset if needed.
pub struct ScratchOutput<const MAX_SIZE: usize = 64> {
    buffer: [u8; MAX_SIZE],
    idx: usize,
}

impl<const MAX_SIZE: usize> Default for ScratchOutput<MAX_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const MAX_SIZE: usize> ScratchOutput<MAX_SIZE> {
    /// Retrieve the currently built buffer
    pub fn result(&self) -> &[u8] {
        &self.buffer[..self.idx]
    }

    /// Reset the buffer, clearing it
    pub fn reset(&mut self) {
        self.idx = 0;
    }

    /// Create a new buffer
    pub const fn new() -> Self {
        Self {
            buffer: [0u8; MAX_SIZE],
            idx: 0,
        }
    }
}

impl<const MAX_SIZE: usize> OutputBuffer for ScratchOutput<MAX_SIZE> {
    type Cursor = usize;

    fn output(&mut self, buf: &[u8]) {
        let area = &mut self.buffer[self.idx..];
        let len = buf.len().clamp(0, area.len());
        area[..len].copy_from_slice(buf);
        self.idx += len;
    }

    fn cur_position(&self) -> Self::Cursor {
        self.idx
    }

    fn update(&mut self, cursor: Self::Cursor, value: u8) {
        if cursor < self.idx {
            if let Some(b) = self.buffer.get_mut(cursor) {
                *b = value;
            }
        }
    }

    fn data_since(&self, cursor: Self::Cursor) -> &[u8] {
        if cursor >= self.idx {
            &[]
        } else {
            &self.buffer[cursor..self.idx]
        }
    }

    fn rollback(&mut self, cursor: Self::Cursor) {
        if cursor <= self.idx {
            self.idx = cursor;
        }
    }
}

#[cfg(feature = "std")]
impl OutputBuffer for Vec<u8> {
    type Cursor = usize;

    fn output(&mut self, buf: &[u8]) {
        self.extend(buf)
    }

    fn cur_position(&self) -> Self::Cursor {
        self.len()
    }

    fn update(&mut self, cursor: Self::Cursor, value: u8) {
        self[cursor] = value;
    }

    fn data_since(&self, cursor: Self::Cursor) -> &[u8] {
        &self[cursor..]
    }

    fn rollback(&mut self, cursor: Self::Cursor) {
        self.truncate(cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_output_rollback_to_start() {
        let mut buf = ScratchOutput::<64>::new();
        let cursor = buf.cur_position();
        buf.output(&[1, 2, 3, 4, 5]);
        assert_eq!(buf.data_since(cursor).len(), 5);
        buf.rollback(cursor);
        assert_eq!(buf.data_since(cursor).len(), 0);
        assert_eq!(buf.cur_position(), 0);
    }

    #[test]
    fn scratch_output_rollback_partial() {
        let mut buf = ScratchOutput::<64>::new();
        buf.output(&[1, 2]);
        let cursor = buf.cur_position();
        buf.output(&[3, 4, 5]);
        assert_eq!(buf.cur_position(), 5);
        buf.rollback(cursor);
        assert_eq!(buf.cur_position(), 2);
        assert_eq!(buf.data_since(0), &[1, 2]);
    }

    #[test]
    fn scratch_output_rollback_noop_for_future_cursor() {
        let mut buf = ScratchOutput::<64>::new();
        buf.output(&[1, 2, 3]);
        buf.rollback(10); // cursor beyond current position
        assert_eq!(buf.cur_position(), 3); // unchanged
    }

    #[cfg(feature = "std")]
    #[test]
    fn vec_rollback_to_start() {
        let mut buf: Vec<u8> = Vec::new();
        let cursor = buf.cur_position();
        buf.output(&[1, 2, 3]);
        buf.rollback(cursor);
        assert!(buf.is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn vec_rollback_partial() {
        let mut buf: Vec<u8> = Vec::new();
        buf.output(&[1, 2]);
        let cursor = buf.cur_position();
        buf.output(&[3, 4, 5]);
        buf.rollback(cursor);
        assert_eq!(buf.as_slice(), &[1, 2]);
    }
}
