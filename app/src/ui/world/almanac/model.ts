import type { Feature, Overlay, WorldFile } from '../../../gen/protocol';

export type FactTag='WORLD'|'PEOPLE'|'CIVILIZATION'|'TERRAIN'|'WATER'|'HYDROLOGY'|'GEOLOGY'|'CLIMATE'|'GEOGRAPHY'|'RELATION';
export type AlmanacMode='overview'|'facts'|'rankings'|'kingdoms';
export type AlmanacFact={text:string;tag:FactTag;feature?:Feature};
export type KingdomSummary={id:number;name:string;capital:Feature|null;population:number;area_cells:number;cities:number;towns:number;villages:number};

const SETTLEMENTS=new Set(['metropolis','city','town','village']);
const REGIONS=new Set(['forest','jungle','taiga','desert','swamp','plains','tundra','glacier','plateau','valley']);
const LAKES=new Set(['lake','salt_lake','salt_flat']);
const LANDMASSES=new Set(['continent','island']);

export const population=(f:Feature)=>Number(/pop\. ([\d,]+)/.exec(f.detail??'')?.[1]?.replace(/,/g,'')??0);
export const drop=(f:Feature)=>Number(/drop ~([\d,]+) ft/.exec(f.detail??'')?.[1]?.replace(/,/g,'')??0);
export const miles=(ft:number)=>ft/5280;
export const areaSqMi=(ft:number)=>(ft*ft)/(5280*5280);
export const fmt=(n:number)=>n.toLocaleString();
export const fmtMiles=(n:number)=>n.toLocaleString(undefined,{maximumFractionDigits:0});
export const distanceMi=(a:Feature,b:Feature)=>Math.hypot(a.x-b.x,a.y-b.y)/5280;
const largest=(xs:Feature[],score:(f:Feature)=>number)=>xs.reduce<Feature|null>((best,f)=>!best||score(f)>score(best)?f:best,null);
const top=(xs:Feature[],score:(f:Feature)=>number,n=20)=>[...xs].sort((a,b)=>score(b)-score(a)||a.name.localeCompare(b.name)).slice(0,n);

export interface AlmanacModel {
 features:Feature[];settlements:Feature[];peaks:Feature[];rivers:Feature[];lakes:Feature[];waterfalls:Feature[];landmasses:Feature[];ranges:Feature[];passes:Feature[];volcanoes:Feature[];regions:Feature[];
 cities:Feature[];towns:Feature[];villages:Feature[];capitals:Feature[];kingdoms:KingdomSummary[];totalPopulation:number;cityPopulationShare:number;capitalPopulationShare:number;namedKinds:number;
 largestSettlement:Feature|null;highestPeak:Feature|null;longestRiver:Feature|null;largestRiverBasin:Feature|null;largestLake:Feature|null;largestLandmass:Feature|null;highestWaterfall:Feature|null;largestRange:Feature|null;
 oceanRivers:Feature[];inlandLakeRivers:Feature[];dryRivers:Feature[];lakeFedRivers:Feature[];terminalLakes:Feature[];flowThroughLakes:Feature[];totalRiverMiles:number;highestOrderRiver:Feature|null;
 activeVolcanoes:Feature[];dormantVolcanoes:Feature[];extinctVolcanoes:Feature[];regionCounts:[string,number][];leadingRegion:[string,number]|null;
 settlementRank:Feature[];peakRank:Feature[];riverRank:Feature[];lakeRank:Feature[];regionRank:[string,number][];facts:AlmanacFact[];
}

function factsFor(world:WorldFile,m:Omit<AlmanacModel,'facts'>):AlmanacFact[]{
 const out:AlmanacFact[]=[];const add=(text:string,tag:FactTag,feature?:Feature)=>out.push({text,tag,feature});
 add(`The generated world contains ${fmt(m.features.length)} named features across ${fmt(m.namedKinds)} feature types.`,'WORLD');
 add(`The map covers ${fmt(world.params.width_mi??0)} × ${fmt(world.params.height_mi??0)} miles.`,'WORLD');
 add(`Its named settlements account for an estimated ${fmt(m.totalPopulation)} people across ${fmt(m.settlements.length)} settlements.`,'PEOPLE');
 add(`${fmt(m.cities.length)} cities or metropolises, ${fmt(m.towns.length)} towns and ${fmt(m.villages.length)} villages make up the settlement network.`,'PEOPLE');
 add(`${fmt(m.capitals.length)} generated capital${m.capitals.length===1?'':'s'} ${m.capitals.length===1?'is':'are'} present.`,'CIVILIZATION');
 add(`Cities and metropolises contain about ${fmt(m.cityPopulationShare)}% of the generated population.`,'CIVILIZATION');
 add(m.capitals.length?`Capital settlements contain about ${fmt(m.capitalPopulationShare)}% of the generated population.`:'No generated settlement is marked as a capital.','CIVILIZATION');
 if(m.largestSettlement)add(`${m.largestSettlement.name} is the largest settlement, with an estimated population of ${fmt(population(m.largestSettlement))}.`,'PEOPLE',m.largestSettlement);
 if(m.highestPeak)add(`${m.highestPeak.name} is the highest named summit at ${fmt(Math.round(m.highestPeak.elev_ft??0))} ft.`,'TERRAIN',m.highestPeak);
 if(m.longestRiver)add(`${m.longestRiver.name} is the longest named river at about ${fmtMiles(m.longestRiver.length_mi??miles(m.longestRiver.extent_ft))} miles.`,'WATER',m.longestRiver);
 if(m.largestRiverBasin)add(`${m.largestRiverBasin.name} drains the largest modeled catchment, about ${fmtMiles(m.largestRiverBasin.drainage_area_mi2??0)} square miles, at stream order ${m.largestRiverBasin.stream_order??1}.`,'HYDROLOGY',m.largestRiverBasin);
 if(m.largestLake)add(`${m.largestLake.name} is the largest generated lake-type feature, about ${fmtMiles(m.largestLake.area_mi2??areaSqMi(m.largestLake.extent_ft))} square miles.`,'WATER',m.largestLake);
 if(m.largestLandmass)add(`${m.largestLandmass.name} is the largest generated landmass by stored extent.`,'GEOGRAPHY',m.largestLandmass);
 if(m.highestWaterfall)add(`${m.highestWaterfall.name} has the greatest named waterfall drop at about ${fmt(drop(m.highestWaterfall))} ft.`,'WATER',m.highestWaterfall);
 if(m.largestRange)add(`${m.largestRange.name} is the longest generated mountain-range feature by stored extent.`,'TERRAIN',m.largestRange);
 for(const [kind,count] of m.regionCounts)add(`${fmt(count)} named ${kind.replace('_',' ')} regions were identified in the generated terrain.`,'CLIMATE');
 const counts=new Map<string,number>();for(const f of m.features)counts.set(f.kind,(counts.get(f.kind)??0)+1);
 for(const [kind,count] of [...counts].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0])))add(`${fmt(count)} generated features use the ${kind.replace('_',' ')} category.`,'WORLD');
 for(const k of m.kingdoms){const share=m.totalPopulation?Math.round(k.population/m.totalPopulation*100):0;add(`${k.name} controls about ${fmt(k.area_cells)} terrain cells and contains an estimated ${fmt(k.population)} people.`,'CIVILIZATION');add(`${k.name} accounts for about ${fmt(share)}% of the generated settlement population.`,'CIVILIZATION');add(`${k.name} has ${fmt(k.cities)} cities, ${fmt(k.towns)} towns and ${fmt(k.villages)} villages.`,'CIVILIZATION');if(k.capital)add(`${k.name} is administered from ${k.capital.name}, its generated capital settlement.`,'CIVILIZATION',k.capital);}
 for(const f of m.settlementRank)add(`${f.name} is among the most populous generated settlements, with an estimated population of ${fmt(population(f))}.`,'PEOPLE',f);
 for(const f of m.peakRank)add(`${f.name} is one of the highest named summits at ${fmt(Math.round(f.elev_ft??0))} ft.`,'TERRAIN',f);
 for(const f of m.riverRank){add(`${f.name} is one of the longest named rivers, measuring about ${fmtMiles(f.length_mi??miles(f.extent_ft))} miles.`,'WATER',f);if(f.stream_order)add(`${f.name} reaches stream order ${fmt(f.stream_order)} in the modeled drainage hierarchy.`,'HYDROLOGY',f);if(f.drainage_area_mi2)add(`${f.name} drains about ${fmtMiles(f.drainage_area_mi2)} square miles.`,'HYDROLOGY',f);if(f.river_mouth)add(`${f.name} is modeled with a ${f.river_mouth} mouth.`,'HYDROLOGY',f);}
 for(const f of m.lakeRank){add(`${f.name} is among the largest named lake-type features, covering about ${fmtMiles(f.area_mi2??areaSqMi(f.extent_ft))} square miles.`,'WATER',f);if(f.has_outlet!==undefined)add(`${f.name} is modeled as a ${f.has_outlet?'flow-through lake with an outlet':'terminal lake without an outlet'}.`,'HYDROLOGY',f);}
 for(const f of m.waterfalls.slice(0,20))add(`${f.name} is a generated waterfall with an estimated drop of ${fmt(drop(f))} ft.`,'WATER',f);
 for(const f of m.ranges.slice(0,20))add(`${f.name} is a generated mountain range spanning about ${fmtMiles(f.length_mi??miles(f.extent_ft))} miles of stored extent.`,'TERRAIN',f);
 for(const kind of ['port','fortress','market','mining','farming','fishing','lumber','herding','oasis'])add(`${fmt(m.settlements.filter(f=>f.kind===kind).length)} settlements were classified as ${kind.replace('_',' ')} centers.`,'CIVILIZATION');
 add(`${fmt(m.settlements.filter(f=>f.coastal).length)} settlements are flagged as coastal.`,'GEOGRAPHY');
 add(`${fmt(m.settlements.filter(f=>f.river).length)} settlements are associated with generated river access.`,'GEOGRAPHY');
 if(m.settlements.length)add(`The settlement network averages about ${fmt(Math.round(m.totalPopulation/m.settlements.length))} people per named settlement.`,'PEOPLE');
 add(`The named river network contains about ${fmtMiles(m.totalRiverMiles)} total river miles across ${fmt(m.rivers.length)} named rivers.`,'HYDROLOGY');
 add(`${fmt(m.oceanRivers.length)} named rivers reach the ocean in the model.`,'HYDROLOGY');
 add(`${fmt(m.inlandLakeRivers.length)} named rivers terminate in generated lakes.`,'HYDROLOGY');
 add(`${fmt(m.lakeFedRivers.length)} named rivers begin from generated lakes.`,'HYDROLOGY');
 add(`${fmt(m.terminalLakes.length)} named lakes are modeled as terminal basins.`,'HYDROLOGY');
 add(`${fmt(m.flowThroughLakes.length)} named lakes have an outlet and through-flow.`,'HYDROLOGY');
 add(`${fmt(m.volcanoes.length)} named volcanic features were generated, including ${fmt(m.activeVolcanoes.length)} active systems.`,'GEOLOGY');
 add(`${fmt(m.ranges.length)} named mountain ranges were extracted from the terrain.`,'TERRAIN');
 add(`${fmt(m.passes.length)} named mountain passes were identified as terrain corridors.`,'TERRAIN');
 add(`${fmt(m.regions.length)} named ecological and geographic regions were extracted.`,'CLIMATE');
 if(m.longestRiver){const n=m.settlements.filter(s=>distanceMi(s,m.longestRiver!)<=50).length;if(n)add(`${fmt(n)} named settlements lie within 50 miles of ${m.longestRiver.name}.`,'RELATION',m.longestRiver);}
 if(m.highestPeak){const n=m.settlements.filter(s=>distanceMi(s,m.highestPeak!)<=50).length;if(n)add(`${fmt(n)} named settlements lie within 50 miles of ${m.highestPeak.name}.`,'RELATION',m.highestPeak);}
 if(m.largestSettlement){const n=m.features.filter(f=>f.id!==m.largestSettlement!.id&&distanceMi(f,m.largestSettlement!)<=25).length;add(`${fmt(n)} other generated feature anchors lie within 25 miles of ${m.largestSettlement.name}.`,'RELATION',m.largestSettlement);}
 return out;
}

export function buildAlmanacModel(world:WorldFile,overlay:Overlay|null,mode:AlmanacMode='overview'):AlmanacModel{
 const features=overlay?.features??[],settlements=features.filter(f=>SETTLEMENTS.has(f.kind)),peaks=features.filter(f=>f.kind==='peak'||f.kind==='volcano'),rivers=features.filter(f=>f.kind==='river'),lakes=features.filter(f=>LAKES.has(f.kind)),waterfalls=features.filter(f=>f.kind==='waterfall'),landmasses=features.filter(f=>LANDMASSES.has(f.kind)),ranges=features.filter(f=>f.kind==='range'),passes=features.filter(f=>f.kind==='pass'),volcanoes=features.filter(f=>f.kind==='volcano'),regions=features.filter(f=>REGIONS.has(f.kind));
 const cities=settlements.filter(f=>f.kind==='city'||f.kind==='metropolis'),towns=settlements.filter(f=>f.kind==='town'),villages=settlements.filter(f=>f.kind==='village'),capitals=settlements.filter(f=>f.political_rank==='capital'||(f.detail??'').includes(', capital'));
 const km=new Map<number,KingdomSummary>();for(const f of settlements){if(f.kingdom_id===undefined)continue;const k=km.get(f.kingdom_id)??{id:f.kingdom_id,name:f.kingdom_name??`Kingdom ${f.kingdom_id+1}`,capital:null,population:0,area_cells:0,cities:0,towns:0,villages:0};k.population+=population(f);if(f.political_rank==='capital')k.capital=f;if(f.kind==='metropolis'||f.kind==='city')k.cities++;else if(f.kind==='town')k.towns++;else k.villages++;km.set(f.kingdom_id,k);}
 for(const k of overlay?.kingdoms??[]){const e=km.get(k.id);if(e)e.area_cells=k.area_cells;else km.set(k.id,{id:k.id,name:k.name,capital:null,population:k.population,area_cells:k.area_cells,cities:k.cities,towns:k.towns,villages:k.villages});}
 const kingdoms=[...km.values()].sort((a,b)=>b.population-a.population||a.name.localeCompare(b.name)),totalPopulation=settlements.reduce((n,f)=>n+population(f),0),cityPopulation=cities.reduce((n,f)=>n+population(f),0),capitalPopulation=capitals.reduce((n,f)=>n+population(f),0);
 const regionCounts=[...regions.reduce((m,f)=>m.set(f.kind,(m.get(f.kind)??0)+1),new Map<string,number>())].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0]));
 const needsRankings=mode==='rankings'||mode==='facts';
 const settlementRank=needsRankings?top(settlements,population):[],peakRank=needsRankings?top(peaks,f=>f.elev_ft??0):[],riverRank=needsRankings?top(rivers,f=>f.length_mi??miles(f.extent_ft)):[],lakeRank=needsRankings?top(lakes,f=>f.extent_ft):[];
 const base={features,settlements,peaks,rivers,lakes,waterfalls,landmasses,ranges,passes,volcanoes,regions,cities,towns,villages,capitals,kingdoms,totalPopulation,cityPopulationShare:totalPopulation?Math.round(cityPopulation/totalPopulation*100):0,capitalPopulationShare:totalPopulation?Math.round(capitalPopulation/totalPopulation*100):0,namedKinds:new Set(features.map(f=>f.kind)).size,largestSettlement:largest(settlements,population),highestPeak:largest(peaks,f=>f.elev_ft??0),longestRiver:largest(rivers,f=>f.length_mi??miles(f.extent_ft)),largestRiverBasin:largest(rivers,f=>f.drainage_area_mi2??0),largestLake:largest(lakes,f=>f.area_mi2??areaSqMi(f.extent_ft)),largestLandmass:largest(landmasses,f=>f.extent_ft),highestWaterfall:largest(waterfalls,drop),largestRange:largest(ranges,f=>f.extent_ft),oceanRivers:rivers.filter(f=>f.river_mouth==='ocean'),inlandLakeRivers:rivers.filter(f=>f.river_mouth==='lake'),dryRivers:rivers.filter(f=>f.river_mouth==='dry'),lakeFedRivers:rivers.filter(f=>f.source_lake_id!==undefined),terminalLakes:lakes.filter(f=>f.has_outlet===false),flowThroughLakes:lakes.filter(f=>f.has_outlet===true),totalRiverMiles:rivers.reduce((n,f)=>n+(f.length_mi??miles(f.extent_ft)),0),highestOrderRiver:largest(rivers,f=>f.stream_order??0),activeVolcanoes:volcanoes.filter(f=>(f.detail??'').startsWith('active ')),dormantVolcanoes:volcanoes.filter(f=>(f.detail??'').startsWith('dormant ')),extinctVolcanoes:volcanoes.filter(f=>(f.detail??'').startsWith('extinct ')),regionCounts,leadingRegion:regionCounts[0]??null,settlementRank,peakRank,riverRank,lakeRank,regionRank:needsRankings?regionCounts.slice(0,20):[]};
 return {...base,facts:mode==='facts'&&overlay?factsFor(world,base):[]};
}