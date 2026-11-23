//! Array initializer layout, shared by semantic lowering and both target backends.
use crate::{Expr,Initializer};

pub fn element_count(dimensions:&[usize])->Result<usize,String> {
    if dimensions.is_empty(){return Err("array shape needs at least one dimension".into());}
    let count=dimensions.iter().try_fold(1usize,|n,&d|n.checked_mul(d).ok_or("array shape overflow"))?;
    let bytes=count.checked_mul(4).ok_or("array byte size overflow")?;
    if bytes>u32::MAX as usize{return Err("array exceeds RV32 address size".into());}
    Ok(count)
}

/// None means an implicit zero. Expressions retain their order and are not evaluated here.
pub fn flatten_initializer(dimensions:&[usize],initializer:&Initializer)->Result<Vec<Option<Expr>>,String> {
    let count=element_count(dimensions)?;let mut output=vec![None;count];
    fill(dimensions,initializer,&mut output,0)?;Ok(output)
}
fn fill(dimensions:&[usize],initializer:&Initializer,output:&mut [Option<Expr>],base:usize)->Result<(),String> {
    let Initializer::List(items)=initializer else{return Err("array initializer must be a list".into());};
    let count=element_count(dimensions)?;let mut cursor=0usize;
    if count==0 {
        if items.is_empty(){return Ok(());}
        if dimensions.len()<2 || items.len()>dimensions[0]{return Err("too many zero-sized subarray initializers".into());}
        for item in items{fill(&dimensions[1..],item,output,base)?;}
        return Ok(());
    }
    for item in items {
        match item {
            Initializer::Scalar(expr)=>{
                if cursor>=count{return Err("too many array initializer elements".into());}
                output[base+cursor]=Some(expr.clone());cursor+=1;
            }
            Initializer::List(_)=>{
                // The longest trailing subarray aligned at this cursor owns the nested braces.
                let mut child=None;
                for depth in 1..dimensions.len() {
                    let size=element_count(&dimensions[depth..])?;
                    if size!=0 && cursor%size==0 {child=Some((depth,size));break;}
                }
                let (depth,size)=child.ok_or("nested initializer is not aligned to a subarray")?;
                if cursor.checked_add(size).is_none_or(|end|end>count){return Err("too many subarray initializers".into());}
                fill(&dimensions[depth..],item,output,base+cursor)?;cursor+=size;
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
 use super::*;
 fn from_source(shape:&str,initializer:&str)->crate::Definition {
  let function=crate::parse(&format!("int main(){{int a{shape}={initializer};return 0;}}")).unwrap();
  let crate::BlockItem::Declaration{mut definitions,..}=function.items.into_iter().next().unwrap() else{panic!()};definitions.remove(0)
 }
 fn values(shape:&[usize],initializer:&str)->Vec<i32> {
  let declaration=from_source(&"[1]".repeat(shape.len()),initializer);
  flatten_initializer(shape,declaration.initializer.as_ref().unwrap()).unwrap().into_iter().map(|x|match x{None=>0,Some(Expr::Number(n))=>n,_=>panic!()}).collect()
 }
 #[test]
 fn official_mixed_and_empty_subarray_examples(){
  assert_eq!(values(&[3,2],"{{1,2},{3},{5}}"),vec![1,2,3,0,5,0]);
  assert_eq!(values(&[3,2],"{1,2,{3},5}"),vec![1,2,3,0,5,0]);
  assert_eq!(values(&[3,2],"{{},{3,4},5,6}"),vec![0,0,3,4,5,6]);
  assert_eq!(values(&[2,3],"{1,2}"),vec![1,2,0,0,0,0]);
  assert_eq!(values(&[2,3,4],"{1,2,3,4,{5},{6},{7,8}}"),vec![1,2,3,4,5,0,0,0,6,0,0,0,7,8,0,0,0,0,0,0,0,0,0,0]);
 }
 #[test]
 fn misaligned_overfull_and_wrong_scalar_initializers_fail(){
  for (shape,initializer) in [(vec![2,2,2],"{{},1,{}}"),(vec![2,3,4],"{1,2,{3}}"),(vec![1,2],"{1,2,3}"),(vec![2],"{{1},2}"),(vec![2,2],"{{1,2,3}}"),(vec![2],"3")] {
   let declaration=from_source("[1]",initializer);
   assert!(flatten_initializer(&shape,declaration.initializer.as_ref().unwrap()).is_err(),"{shape:?} {initializer}");
  }
 }
 #[test]
 fn zero_extent_overflow_and_expression_order(){
  assert!(values(&[0],"{}").is_empty());assert!(values(&[2,0],"{{},{}}").is_empty());
  assert!(element_count(&[usize::MAX,2]).is_err());assert!(element_count(&[u32::MAX as usize]).is_err());
  let declaration=from_source("[2][2]","{{f(1)},{f(2)}}");
  let actual=flatten_initializer(&[2,2],declaration.initializer.as_ref().unwrap()).unwrap();
  assert_eq!(actual,vec![Some(Expr::Call{name:"f".into(),arguments:vec![Expr::Number(1)]}),None,Some(Expr::Call{name:"f".into(),arguments:vec![Expr::Number(2)]}),None]);
 }
}
