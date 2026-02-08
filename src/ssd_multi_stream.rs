use super::block::*;
use super::mapping::*;
use super::page::*;
use log::{info, debug, warn, trace};
pub struct SSD {
    blocks: Vec<Block>,
    mapping_table: MappingTable,
    hot_block_idx: usize,
    cold_block_idx: usize,
    user_write_cnt: u64,
    nand_write_cnt: u64,
    gap_threshold: u32
}

#[derive(Clone, Copy)]
pub enum STREAM {
    HOT,
    COLD
}

impl SSD {
    pub fn new(num_blocks: usize, num_lbas: usize) -> Self {

        let mut blocks = Vec::new();
        for i in 0..num_blocks {
            blocks.push(Block::new(i as u32));
        }
        
        SSD {
            blocks,
            mapping_table: MappingTable::new(num_lbas),
            hot_block_idx: 0,
            cold_block_idx: num_blocks-1,
            user_write_cnt: 0,
            nand_write_cnt: 0,
            gap_threshold: 5
        }
    }

    
    pub fn write(&mut self, lba: usize, data: u32, stream: STREAM) -> Result<(), String> {
        self.user_write_cnt += 1; // 사용자가 요청했으므로 증가

        while self.count_free_blocks() == 0 {
            self.gc()?;
        }
        // 여기서 바꿀 수 있어야 한다. 
        if let Err(_) = self.write_internal(lba, data, stream) {
            if let Some(next_idx) = self.find_next_free_block() {
                
                match stream {
                    STREAM::HOT => {
                        debug!("Switching Active Hot Block: {} -> {}", self.hot_block_idx, next_idx);
                        self.hot_block_idx = next_idx;
                    },
                    STREAM::COLD => {
                        debug!("Switching Active Cold Block: {} -> {}", self.cold_block_idx, next_idx);
                        self.cold_block_idx = next_idx;
                    }
                }
                self.write_internal(lba, data, stream)?;
            } else {
                return Err("Fatal Error: SSD is Full!".to_string());
            }
        }
        Ok(())
    }

    fn write_internal(&mut self, lba: usize, data: u32, stream: STREAM) -> Result<(), String> {
        let active_block_idx = match stream {
            STREAM::HOT => self.hot_block_idx,
            STREAM::COLD => self.cold_block_idx
        };
        let block = &mut self.blocks[active_block_idx];
 
        let mut target_page = None;
        for page_offset in 0..PAGES_PER_BLOCK {
            if block.read(page_offset).state == PageState::Free {
                
                self.nand_write_cnt += 1; 
                block.program(page_offset, data);
                target_page = Some(page_offset);
                break;
            }
        }

        if let Some(page_offset) = target_page {
            let new_pba = PhysicalAddress {
                block_id: self.blocks[active_block_idx].id,
                page_offset,
            };

            if let Some(old_pba) = self.mapping_table.update(lba, new_pba) {
                let old_blk_idx = old_pba.block_id as usize;
                
                if old_blk_idx < self.blocks.len() {
                    self.blocks[old_blk_idx].pages[old_pba.page_offset].state = PageState::Invalid;
                    debug!("  -> Invalidated Old Data: Block {} Page {}", old_blk_idx, old_pba.page_offset);
                }
            }
            Ok(())
        } else {
            Err("Active block is full".to_string())
        }
    }
    
    pub fn gc(&mut self) -> Result<(), String> {
        info!("\n[GC] Started! (Free blocks: {})", self.count_free_blocks());

        let stat = self.compute_wear_metrics();
        let migration_stream = STREAM::COLD;
        let victim_idx = if stat.gap > self.gap_threshold {
            info!("[WL] Triggered! Gap: {} (Max: {}, Min: {})", stat.gap, stat.max, stat.min);
            
            let mut target = None;
            for (i, block) in self.blocks.iter().enumerate() {
                if block.erase_count == stat.min && i != self.hot_block_idx && i != self.cold_block_idx {
                    target = Some(i);
                    break;
                }
            }
            
            match target {
                Some(idx) => {
                    info!("[WL] Forcing Cold Block {} to be cleaned.", idx);
                    idx
                },
                None => {
                    return Err("WL Triggered but Cold Block is Active".to_string());
                }
            }
        } else {
            let mut target = None;
            let mut min_valid_count = usize::MAX;

            for (i, block) in self.blocks.iter().enumerate() {
                // 현재 쓰고 있는 블록이나 이미 빈 블록은 제외
                if i == self.hot_block_idx || i == self.cold_block_idx || block.state == BlockState::Free {
                    continue;
                }

                let valid_cnt = block.count_valid_pages();
                if valid_cnt < min_valid_count {
                    min_valid_count = valid_cnt;
                    target = Some(i);
                }
            }

            match target {
                Some(idx) => idx,
                None => return Err("Failed to find victim block! (SSD might be clean)".to_string()),
            }
        };

    let valid_pages_cnt = self.blocks[victim_idx].count_valid_pages();
    debug!("[GC] Selected Victim: Block {} (Valid Pages: {})", victim_idx, valid_pages_cnt);
        // 2. 유효 페이지 대피 (Migration)
        for page_idx in 0..PAGES_PER_BLOCK {
            let is_valid = self.blocks[victim_idx].pages[page_idx].state == PageState::Valid;
    
            if is_valid {
                let data = self.blocks[victim_idx].pages[page_idx].content;
                let lba_opt = self.find_lba_by_pba(victim_idx as u32, page_idx);
    
                if let Some(target_lba) = lba_opt {
                    // ✅ 루프: Active Block이 또 꽉 찰 때까지 계속 전환
                    loop {
                        match self.write_internal(target_lba, data, migration_stream) {
                            Ok(()) => break,  // 성공! 루프 탈출
                            Err(_) => {
                                // Active Block 꽉 찼음
                                match self.find_next_free_block() {
                                    Some(next_idx) => {
                                        self.cold_block_idx = next_idx;
                                    }
                                    None => {
                                        // 정말 더 이상 공간 없음
                                        return Err(
                                            format!("Fatal: No space left during GC migration at page {}", page_idx)
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    
        self.blocks[victim_idx].erase();
        info!("[GC] Erased Block {}. GC Finished.\n", victim_idx);
        Ok(())
    }
    
    // [보조 함수] 빈 블록 개수 세기 (GC 트리거 조건용)
    fn count_free_blocks(&self) -> usize {
        self.blocks.iter().filter(|b| b.state == BlockState::Free).count()
    }

    fn find_next_free_block(&self) -> Option<usize> {
        for (i, block) in self.blocks.iter().enumerate() {
            // 현재 쓰고 있는 블록은 제외하고 찾기
            if block.state == BlockState::Free && i != self.hot_block_idx && i != self.cold_block_idx {
                return Some(i);
            }
        }
        None
    }
    
    // MappingTable 구현에 따라 entries 접근 방식이 다를 수 있음 (여기선 Vec 직접 접근 가정)
    fn find_lba_by_pba(&self, block_id: u32, page_offset: usize) -> Option<usize> {
        // MappingTable의 entries 필드가 pub이어야 합니다.
        // 만약 entries() 메서드를 쓰신다면 그대로 두셔도 됩니다.
        for (lba, entry) in self.mapping_table.entries().iter().enumerate() {
            if let Some(pba) = entry {
                if pba.block_id == block_id && pba.page_offset == page_offset {
                    return Some(lba);
                }
            }
        }
        None
    }

    pub fn get_waf(&self) -> f64 {
        if self.user_write_cnt == 0 { return 0.0 }
        self.nand_write_cnt as f64 / self.user_write_cnt as f64 
    }

    pub fn print_blocks(&self) {
        for block in &self.blocks {
            println!("{:?}", block);
        }
        println!("===============================")
    }
    // erase의 평균과 
    pub fn compute_wear_metrics(&self) -> WearStats {
        let mut min = u32::MAX;
        let mut max = 0;
        let sum = self.blocks.iter().fold(0, |acc, x| {
            let cnt = x.erase_count;
            if cnt < min {min = cnt;}
            if cnt > max {max = cnt;}
            acc + x.erase_count}
        );
        WearStats { min: min, max: max, avg: sum as f64/ self.blocks.len() as f64, gap: max - min }
    }

}