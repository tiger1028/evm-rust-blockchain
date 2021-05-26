//! Allows to listen to runtime events.

use crate::{Context, Opcode, Stack, Memory, Capture, ExitReason, Trap};
use primitive_types::{H160, H256};

environmental::environmental!(listener: dyn EventListener + 'static);

pub trait EventListener {
    fn event(
        &mut self,
        event: Event
    );
}

#[derive(Debug, Copy, Clone)]
pub enum Event<'a> {
    Step {
        context: &'a Context,
        opcode: Opcode,
        position: &'a Result<usize, ExitReason>,
        stack: &'a Stack,
        memory: &'a Memory
    },
    StepResult {
        result: &'a Result<(), Capture<ExitReason, Trap>>,
        return_value: &'a [u8],
    },
    SLoad {
        address: H160,
        index: H256,
        value: H256
    },
    SStore {
        address: H160,
        index: H256,
        value: H256
    },
}

impl<'a> Event<'a> {
    pub(crate) fn emit(self) {
        listener::with(|listener| listener.event(self));
    }
}

/// Run closure with provided listener.
pub fn using<R, F: FnOnce() -> R>(
    new: &mut (dyn EventListener + 'static),
    f: F
) -> R {
    listener::using(new, f)
}
