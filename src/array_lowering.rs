//! Array semantic checks and typed pointer lowering. Child module of the shared IR.
use super::*;
use crate::Definition;
impl Builder {
    pub(super) fn parameter_shapes(&self,function:&Function)->Result<Vec<Option<Vec<usize>>>,String> {
        if function.parameters.len()!=function.parameter_dimensions.len(){return Err("inconsistent parameter metadata".into());}
        let mut context=Builder::new(self.scopes[0].clone(),self.signatures.clone(),false);let mut types=Vec::new();
        for (name,dimensions) in function.parameters.iter().zip(&function.parameter_dimensions) {
            if context.current().contains_key(name){return Err(format!("duplicate parameter: {name}"));}
            types.push(dimensions.as_ref().map(|dims|context.dimension_values(dims)).transpose()?);
            // Earlier parameters shadow outer constants while later declarators are checked.
            context.current().insert(name.clone(),Binding::Variable(usize::MAX));
        }
        Ok(types)
    }
    pub(super) fn dimension_values(&self,expressions:&[Expr])->Result<Vec<usize>,String> {
        let dimensions=expressions.iter().map(|expr|{
            self.validate(expr,true)?;let value=self.constant(expr)?;
            usize::try_from(value).map_err(|_|"negative array dimension".into())
        }).collect::<Result<Vec<_>,String>>()?;
        if !dimensions.is_empty(){element_count(&dimensions)?;}
        Ok(dimensions)
    }
    fn array_info(&self,name:&str,indices:&[Expr])->Result<ArrayBinding,String> {
        let Some(Binding::Array(info))=self.lookup(name) else{return Err(format!("not an array: {name}"));};
        if indices.len()>info.shape.len(){return Err(format!("too many array indices: {name}"));}
        for index in indices{self.validate(index,false)?;}
        Ok(info)
    }
    pub(super) fn validate_array_element(&self,name:&str,indices:&[Expr])->Result<(),String> {
        let info=self.array_info(name,indices)?;
        if indices.len()!=info.shape.len(){return Err("subarray is only valid as an array argument".into());}
        Ok(())
    }
    pub(super) fn array_argument_type(&self,expr:&Expr)->Result<Vec<usize>,String> {
        let (name,indices)=match expr{Expr::Variable(name)=>(name.as_str(),&[][..]),Expr::Index{name,indices}=>(name.as_str(),indices.as_slice()),_=>return Err("expected an array argument".into())};
        let info=self.array_info(name,indices)?;
        if indices.len()==info.shape.len(){return Err("scalar element passed as array".into());}
        Ok(info.shape[indices.len()+1..].to_vec())
    }
    pub(super) fn lower_argument(&mut self,expr:Expr,pointer:&Option<Vec<usize>>)->Result<Operand,String> {
        if pointer.is_some(){
            match expr {Expr::Variable(name)=>self.array_address(&name,vec![],true),Expr::Index{name,indices}=>self.array_address(&name,indices,true),_=>Err("expected array address".into())}
        }else{self.lower_validated(expr)}
    }
    fn pointer(&mut self,base:PointerBase,index:Operand,stride:usize,parameter_step:bool)->PointerBase {
        let destination=self.next;self.next+=1;self.emit(Instruction::ElementPointer{destination,base,index,stride,parameter_step});PointerBase::Temporary(destination)
    }
    fn array_address(&mut self,name:&str,indices:Vec<Expr>,decay:bool)->Result<Operand,String> {
        let info=self.array_info(name,&indices)?;
        if decay && indices.len()==info.shape.len(){return Err("cannot decay scalar element".into());}
        if self.active.is_none(){return Ok(Operand::Integer(0));}
        let parameter=matches!(info.storage,ArrayStorage::Parameter(_));
        let mut base=match info.storage {
            ArrayStorage::Local(slot)=>PointerBase::Slot(slot),ArrayStorage::Global(global)=>PointerBase::Global(global),
            ArrayStorage::Parameter(slot)=>{let Operand::Temporary(id)=self.load_slot(slot) else{unreachable!()};PointerBase::Temporary(id)},
        };
        let count=indices.len();
        for (depth,index) in indices.into_iter().enumerate(){
            let index=self.lower_validated(index)?;
            let stride=info.shape[depth+1..].iter().product::<usize>()*4;
            base=self.pointer(base,index,stride,parameter&&depth==0);
        }
        if decay && !(parameter&&count==0){
            let stride=info.shape[count+1..].iter().product::<usize>()*4;
            base=self.pointer(base,Operand::Integer(0),stride,false);
        }
        let PointerBase::Temporary(id)=base else{return Err("array address needs an element or decay".into());};
        Ok(Operand::Temporary(id))
    }
    pub(super) fn read_array(&mut self,name:&str,indices:Vec<Expr>)->Result<Operand,String> {
        self.validate_array_element(name,&indices)?;let pointer=self.array_address(name,indices,false)?;
        let destination=self.next;self.next+=1;self.emit(Instruction::IndirectLoad{destination,pointer});Ok(Operand::Temporary(destination))
    }
    pub(super) fn assign_array(&mut self,name:&str,indices:Vec<Expr>,value:Expr)->Result<(),String> {
        let info=self.array_info(name,&indices)?;
        if info.readonly{return Err(format!("assignment to constant array: {name}"));}
        self.validate_array_element(name,&indices)?;
        let pointer=self.array_address(name,indices,false)?;let value=self.lower(value)?;
        self.emit(Instruction::IndirectStore{pointer,value});Ok(())
    }
    pub(super) fn declare_array(&mut self,definition:Definition,constant:bool)->Result<(),String> {
        if self.current().contains_key(&definition.name){return Err(format!("duplicate identifier: {}",definition.name));}
        let shape=self.dimension_values(&definition.dimensions)?;
        let slot=self.next_slot;self.next_slot+=1;self.emit(Instruction::AllocateArray{slot,shape:shape.clone()});
        let name=definition.name;
        self.current().insert(name.clone(),Binding::Array(ArrayBinding{storage:ArrayStorage::Local(slot),shape:shape.clone(),readonly:constant}));
        let Some(initializer)=definition.initializer else{
            if constant{return Err("constant array requires initializer".into());}return Ok(());
        };
        let flat=flatten_initializer(&shape,&initializer)?;
        for (offset,expr) in flat.into_iter().enumerate(){
            let value=if let Some(expr)=expr {
                if constant{self.validate(&expr,true)?;Operand::Integer(self.constant(&expr)?)}else{self.lower(expr)?}
            }else{Operand::Integer(0)};
            let mut remaining=offset;let mut indices=Vec::new();
            for depth in 0..shape.len(){let width=shape[depth+1..].iter().product::<usize>();indices.push(Expr::Number((remaining/width) as i32));remaining%=width;}
            let pointer=self.array_address(&name,indices,false)?;self.emit(Instruction::IndirectStore{pointer,value});
        }
        Ok(())
    }
}
