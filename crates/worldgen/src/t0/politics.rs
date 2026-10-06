//! Deterministic political geography built from generated settlements.
use serde::Serialize;
use super::flood::neighbors;
use super::names::{NameKind, Namer};
use super::settle::{Settlement, Tier};
use crate::World;

#[derive(Clone, Debug, Serialize)]
pub struct Kingdom { pub id: u16, pub name: String, pub capital: usize, pub population: u64, pub area_cells: u32, pub cities: u32, pub towns: u32, pub villages: u32 }
#[derive(Clone, Debug, Serialize)]
pub struct Politics { pub kingdoms: Vec<Kingdom>, pub kingdom_of: Vec<u16> }

pub fn assign(world: &World, w: usize, h: usize, land: &[bool], settlements: &mut [Settlement]) -> Politics {
    let n=w*h;
    let mut capitals: Vec<usize>=settlements.iter().enumerate().filter_map(|(i,s)|(s.tier==Tier::Metropolis).then_some(i)).collect();
    capitals.sort_by(|&a,&b| settlements[b].population.cmp(&settlements[a].population).then(a.cmp(&b)));
    if capitals.is_empty() { if let Some((i,_))=settlements.iter().enumerate().max_by_key(|(_,s)|s.population){capitals.push(i);} }
    let mut comp=vec![u32::MAX;n]; let mut next=0u32;
    for start in 0..n { if !land[start]||comp[start]!=u32::MAX{continue} let mut q=vec![start]; comp[start]=next; let mut head=0; while head<q.len(){let k=q[head];head+=1;for (nb,_) in neighbors(w,h,k){if land[nb]&&comp[nb]==u32::MAX{comp[nb]=next;q.push(nb);}}} next+=1; }
    let cap_comp:Vec<u32>=capitals.iter().map(|&i|comp[settlements[i].cell]).collect();
    let mut kingdom_of=vec![u16::MAX;n];
    for cell in 0..n { if !land[cell]{continue} let c=comp[cell]; let mut best=None; for (ki,&si) in capitals.iter().enumerate(){if cap_comp.iter().any(|&x|x==c)&&cap_comp[ki]!=c{continue} let a=settlements[si].cell;let dx=(a%w)as f64-(cell%w)as f64;let dy=(a/w)as f64-(cell/w)as f64;let d=dx*dx+dy*dy;if best.is_none_or(|(bd,bi):(f64,usize)|d<bd||(d==bd&&ki<bi)){best=Some((d,ki));}} if let Some((_,ki))=best{kingdom_of[cell]=ki as u16;} }
    let mut namer=Namer::new(world.stream("t0.kingdom.names"));
    let mut kingdoms=Vec::with_capacity(capitals.len());
    for (ki,&cap) in capitals.iter().enumerate(){let s=&settlements[cap];let culture=((s.cell as u64).wrapping_mul(0x9e3779b97f4a7c15)as usize)%5;kingdoms.push(Kingdom{id:ki as u16,name:namer.name(NameKind::Kingdom,culture),capital:cap,population:0,area_cells:0,cities:0,towns:0,villages:0});}
    for s in settlements.iter_mut(){s.capital=false;s.kingdom_id=kingdom_of[s.cell].min(kingdoms.len().saturating_sub(1) as u16);}
    for k in &kingdoms { if let Some(s)=settlements.get_mut(k.capital){s.capital=true;s.kingdom_id=k.id;} }
    for cell in 0..n {let id=kingdom_of[cell];if id!=u16::MAX{kingdoms[id as usize].area_cells+=1;}}
    for s in settlements.iter(){let k=&mut kingdoms[s.kingdom_id as usize];k.population+=s.population as u64;match s.tier{Tier::Metropolis|Tier::City=>k.cities+=1,Tier::Town=>k.towns+=1,Tier::Village=>k.villages+=1}}
    Politics{kingdoms,kingdom_of}
}
