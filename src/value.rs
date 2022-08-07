use crate::{
    basic_block::BasicBlock,
    context::{Context, Ptr},
    operation::Operation,
    use_def_lists::{Use, UseRef},
};

// Container for a Def.
pub trait Value {
    // If this is an OpResult, return the defining Operation.
    fn get_defining_op(&self) -> Option<Ptr<Operation>>;
    // If this is a BlockArgument, or if the defining Operation is in a block.
    fn get_parent_block(&self) -> Option<Ptr<BasicBlock>>;
    // This definition is i'th result of the operation or i'th block argument.
    fn get_def_index(&self) -> usize;
    // Get this value's uses.
    fn get_uses(&self) -> &Vec<UseRef>;
    // Get this value's uses (mut).
    fn get_uses_mut(&mut self) -> &mut Vec<UseRef>;
    // Given a UseRef, add it as a use of this value.
    // Returns the new Use that can be used to build an operand.
    fn add_use(&mut self, r#use: UseRef) -> Use;

    // Replace all uses of this value with @new_val.
    // When this fn finishes, self will not have any uses.
    fn replace_all_uses_with(&mut self, ctx: &mut Context, new_val: &mut dyn Value) {
        let uses = self.get_uses_mut();
        for r#use in uses {
            let new_use = new_val.add_use(*r#use);
            r#use.get_operand_mut(ctx).replace_def(new_use);
        }
    }
}
