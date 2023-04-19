//! [BasicBlock] is an list of [Operation]s.

use rustc_hash::FxHashMap;

use crate::{
    attribute::AttrObj,
    common_traits::{DisplayWithContext, Named, Verify},
    context::{ArenaCell, ArenaObj, Context, Ptr},
    debug_info::get_block_arg_name,
    error::CompilerError,
    linked_list::{ContainsLinkedList, LinkedList},
    operation::Operation,
    r#type::TypeObj,
    region::Region,
    use_def_lists::{DefNode, Value},
    with_context::AttachContext,
};

/// Argument to a [BasicBlock]
pub struct BlockArgument {
    /// The def containing the list of this argument's uses.
    pub(crate) def: DefNode<Value>,
    /// A [Ptr] to the [BasicBlock] of which this is an argument.
    def_block: Ptr<BasicBlock>,
    /// Index of this argument in the block's list of arguments.
    arg_idx: usize,
    /// The [Type](crate::type::Type) of this argument.
    ty: Ptr<TypeObj>,
}

impl BlockArgument {
    /// A [Ptr] to the [BasicBlock] of which this is an argument.
    pub fn get_def_block(&self) -> Ptr<BasicBlock> {
        self.def_block
    }

    /// Index of this argument in the block's list of arguments.
    pub fn get_arg_idx(&self) -> usize {
        self.arg_idx
    }

    /// Get the [Type](crate::type::Type) of this block argument.
    pub fn get_type(&self) -> Ptr<TypeObj> {
        self.ty
    }
}

impl Named for BlockArgument {
    fn get_name(&self, ctx: &Context) -> String {
        get_block_arg_name(ctx, self.get_def_block(), self.arg_idx).unwrap_or_else(|| {
            let mut name = self.def_block.deref(ctx).get_name(ctx);
            name.push_str(&format!("[{}]", self.arg_idx));
            name
        })
    }
}

impl From<&BlockArgument> for Value {
    fn from(value: &BlockArgument) -> Self {
        Value::BlockArgument {
            block: value.def_block,
            arg_idx: value.arg_idx,
        }
    }
}

impl AttachContext for BlockArgument {}

impl DisplayWithContext for BlockArgument {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}:{}",
            self.get_name(ctx),
            self.get_type().with_ctx(ctx)
        )
    }
}

/// [Operation]s contained in this [BasicBlock]
pub struct OpsInBlock {
    first: Option<Ptr<Operation>>,
    last: Option<Ptr<Operation>>,
}

impl OpsInBlock {
    fn new_empty() -> OpsInBlock {
        OpsInBlock {
            first: None,
            last: None,
        }
    }
}

/// An iterator for the [Operation]s in this [BasicBlock].
/// This is created by [BasicBlock::iter()].
pub struct Iter<'a> {
    next: Option<Ptr<Operation>>,
    next_back: Option<Ptr<Operation>>,
    ctx: &'a Context,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Ptr<Operation>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next.map(|curr| {
            if curr
                == self
                    .next_back
                    .expect("Some(next) => Some(next_back) violated")
            {
                self.next = None;
                self.next_back = None;
            } else {
                self.next = curr.deref(self.ctx).block_links.next_op;
            }
            curr
        })
    }

    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.next_back.map(|curr| {
            if curr == self.next.expect("Some(next_back) => Some(next) violated") {
                self.next_back = None;
                self.next = None;
            } else {
                self.next_back = curr.deref(self.ctx).block_links.prev_op;
            }
            curr
        })
    }
}

/// Links a [BasicBlock] with other blocks and the container [Region].
pub struct RegionLinks {
    /// Parent region of this block.
    pub parent_region: Option<Ptr<Region>>,
    /// The next block in the region's list of block.
    pub next_block: Option<Ptr<BasicBlock>>,
    /// The previous block in the region's list of blocks.
    pub prev_block: Option<Ptr<BasicBlock>>,
}

impl RegionLinks {
    pub fn new_unlinked() -> RegionLinks {
        RegionLinks {
            parent_region: None,
            next_block: None,
            prev_block: None,
        }
    }
}

/// A basic block contains a list of [Operation]s. It may have [arguments](BlockArgument).
pub struct BasicBlock {
    pub(crate) self_ptr: Ptr<BasicBlock>,
    pub(crate) label: Option<String>,
    pub(crate) ops_list: OpsInBlock,
    pub(crate) args: Vec<BlockArgument>,
    pub(crate) preds: DefNode<Ptr<BasicBlock>>,
    /// Links to the parent [Region] and
    /// previous and next [BasicBlock]s in the block.
    pub region_links: RegionLinks,
    /// A dictionary of attributes.
    pub attributes: FxHashMap<&'static str, AttrObj>,
}

impl Named for BasicBlock {
    fn get_name(&self, _ctx: &Context) -> String {
        self.label
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.self_ptr.make_name("block"))
    }
}

impl BasicBlock {
    /// Get an iterator to the operations inside this block.
    pub fn iter<'a>(&self, ctx: &'a Context) -> Iter<'a> {
        Iter {
            next: self.ops_list.first,
            next_back: self.ops_list.last,
            ctx,
        }
    }

    /// Create a new Basic Block.
    pub fn new(
        ctx: &mut Context,
        label: Option<String>,
        arg_types: Vec<Ptr<TypeObj>>,
    ) -> Ptr<BasicBlock> {
        let f = |self_ptr: Ptr<BasicBlock>| BasicBlock {
            self_ptr,
            label,
            args: vec![],
            ops_list: OpsInBlock::new_empty(),
            preds: DefNode::new(),
            region_links: RegionLinks::new_unlinked(),
            attributes: FxHashMap::default(),
        };
        let newblock = Self::alloc(ctx, f);
        // Let's update the args of the new block. Easier to do it here than during creation.
        let args = arg_types
            .into_iter()
            .enumerate()
            .map(|(arg_idx, ty)| BlockArgument {
                def: DefNode::new(),
                def_block: newblock,
                arg_idx,
                ty,
            })
            .collect();
        newblock.deref_mut(ctx).args = args;
        // We're done.
        newblock
    }

    /// Get idx'th argument as a Value.
    pub fn get_argument(&self, arg_idx: usize) -> Option<Value> {
        self.args.get(arg_idx).map(|arg| arg.into())
    }

    /// Get a reference to the idx'th argument.
    pub(crate) fn get_argument_ref(&self, arg_idx: usize) -> Option<&BlockArgument> {
        self.args.get(arg_idx)
    }

    /// Get a mutable reference to the idx'th argument.
    pub(crate) fn get_argument_mut(&mut self, arg_idx: usize) -> Option<&mut BlockArgument> {
        self.args.get_mut(arg_idx)
    }

    /// Get the number of arguments.
    pub fn get_num_arguments(&self) -> usize {
        self.args.len()
    }
}

impl ContainsLinkedList<Operation> for BasicBlock {
    fn get_head(&self) -> Option<Ptr<Operation>> {
        self.ops_list.first
    }

    fn get_tail(&self) -> Option<Ptr<Operation>> {
        self.ops_list.last
    }

    fn set_head(&mut self, head: Option<Ptr<Operation>>) {
        self.ops_list.first = head;
    }

    fn set_tail(&mut self, tail: Option<Ptr<Operation>>) {
        self.ops_list.last = tail;
    }
}

impl PartialEq for BasicBlock {
    fn eq(&self, other: &Self) -> bool {
        self.self_ptr == other.self_ptr
    }
}

impl LinkedList for BasicBlock {
    type ContainerType = Region;

    fn get_next(&self) -> Option<Ptr<Self>> {
        self.region_links.next_block
    }

    fn get_prev(&self) -> Option<Ptr<Self>> {
        self.region_links.prev_block
    }

    fn set_next(&mut self, next: Option<Ptr<Self>>) {
        self.region_links.next_block = next;
    }

    fn set_prev(&mut self, prev: Option<Ptr<Self>>) {
        self.region_links.prev_block = prev;
    }

    fn get_container(&self) -> Option<Ptr<Self::ContainerType>> {
        self.region_links.parent_region
    }

    fn set_container(&mut self, container: Option<Ptr<Self::ContainerType>>) {
        self.region_links.parent_region = container;
    }
}

impl ArenaObj for BasicBlock {
    fn get_arena(ctx: &Context) -> &ArenaCell<Self> {
        &ctx.basic_blocks
    }
    fn get_arena_mut(ctx: &mut Context) -> &mut ArenaCell<Self> {
        &mut ctx.basic_blocks
    }
    fn dealloc_sub_objects(ptr: Ptr<Self>, ctx: &mut Context) {
        let ops: Vec<_> = ptr.deref_mut(ctx).iter(ctx).collect();
        for op in ops {
            ArenaObj::dealloc(op, ctx);
        }
    }
    fn remove_references(_ptr: Ptr<Self>, _ctx: &mut Context) {
        todo!()
    }

    fn get_self_ptr(&self, _ctx: &Context) -> Ptr<Self> {
        self.self_ptr
    }
}

impl Verify for BasicBlock {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        self.iter(ctx).try_for_each(|op| op.deref(ctx).verify(ctx))
    }
}

impl AttachContext for BasicBlock {}
impl DisplayWithContext for BasicBlock {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}(", self.get_name(ctx))?;
        for arg in self.args.iter() {
            write!(f, "{}", arg.with_ctx(ctx))?;
        }
        writeln!(f, "):")?;
        for op in self.iter(ctx) {
            writeln!(
                f,
                "{}",
                indent::indent_all_by(2, op.with_ctx(ctx).to_string())
            )?;
        }
        Ok(())
    }
}
