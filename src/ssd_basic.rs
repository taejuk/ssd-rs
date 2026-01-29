use super::block::*;
use super::mapping::*;
use super::page::*;

pub struct SSD {
    blocks: Vec<Block>,
    mapping_table: MappingTable,
    active_block_idx: usize,
    user_write_cnt: u64,
    nand_write_cnt: u64,
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
            active_block_idx: 0,
            user_write_cnt: 0,
            nand_write_cnt: 0,
        }
    }

    // [수정 1] 공용 Write 함수: 정책 담당 (사용자 카운트 증가 + GC 트리거 + 위임)
    pub fn write(&mut self, lba: usize, data: u32) -> Result<(), String> {
        self.user_write_cnt += 1; // 사용자가 요청했으므로 증가

        // [방어 로직] 빈 블록이 1개 이하로 남으면 미리 GC를 돌려서 여유 공간 확보 (Reserved Block)
        if self.count_free_blocks() <= 1 {
            self.gc();
        }

        // 실제 쓰기는 internal에게 위임!
        // 만약 internal이 실패하면(Active Block Full), 블록을 바꾸고 다시 시도
        if let Err(_) = self.write_internal(lba, data) {
            // Active Block이 꽉 찼으니 다음 빈 블록 찾기
            if let Some(next_idx) = self.find_next_free_block() {
                //println!("Switching Active Block: {} -> {}", self.active_block_idx, next_idx);
                self.active_block_idx = next_idx;
                
                // 블록 교체 후 재시도 (여기선 무조건 성공해야 함)
                return self.write_internal(lba, data);
            } else {
                // GC를 했는데도 빈 블록이 없다?
                return Err("Fatal Error: SSD is Full!".to_string());
            }
        }
        Ok(())
    }

    // [수정 2] 내부 Write 함수: 실제 동작 담당 (NAND 카운트 증가 + 쓰기 + 매핑)
    // GC는 이 함수를 호출하므로 user_write_cnt가 오르지 않음 (WAF 정확도 상승)
    fn write_internal(&mut self, lba: usize, data: u32) -> Result<(), String> {
        let block = &mut self.blocks[self.active_block_idx];

        // 빈 페이지 찾기
        let mut target_page = None;
        for page_offset in 0..PAGES_PER_BLOCK {
            if block.read(page_offset).state == PageState::Free {
                // [수정] 여기서만 NAND 카운트를 올리면 됨 (GC 상황도 포함되므로)
                self.nand_write_cnt += 1; 
                block.program(page_offset, data);
                target_page = Some(page_offset);
                break;
            }
        }

        if let Some(page_offset) = target_page {
            let new_pba = PhysicalAddress {
                block_id: self.blocks[self.active_block_idx].id,
                page_offset,
            };

            // 매핑 테이블 갱신 및 Old Data 무효화
            if let Some(old_pba) = self.mapping_table.update(lba, new_pba) {
                let old_blk_idx = old_pba.block_id as usize;
                
                // [안전 장치] 혹시 모를 인덱스 에러 방지
                if old_blk_idx < self.blocks.len() {
                    // Block에 invalidate 메서드가 있다고 가정 (직접 접근도 가능)
                    self.blocks[old_blk_idx].pages[old_pba.page_offset].state = PageState::Invalid;
                    println!("  -> Invalidated Old Data: Block {} Page {}", old_blk_idx, old_pba.page_offset);
                }
            }
            Ok(())
        } else {
            // 현재 Active Block이 꽉 참 -> 상위 함수(write)나 GC가 처리하도록 에러 반환
            Err("Active block is full".to_string())
        }
    }

    pub fn gc(&mut self) {
        println!("\n[GC] Started! (Free blocks: {})", self.count_free_blocks());

        let mut victim_idx = None;
        let mut min_valid_count = usize::MAX;

        // 1. 희생자 선정
        for (i, block) in self.blocks.iter().enumerate() {
            // Active Block과 Free Block은 건드리지 않음
            if i == self.active_block_idx || block.state == BlockState::Free {
                continue;
            }

            let valid_cnt = block.count_valid_pages();
            if valid_cnt < min_valid_count {
                min_valid_count = valid_cnt;
                victim_idx = Some(i);
            }
        }

        let victim_idx = match victim_idx {
            Some(idx) => idx,
            None => {
                println!("[GC] Failed to find victim block! (Maybe SSD is clean)");
                return;
            }
        };

        println!("[GC] Selected Victim: Block {} (Valid Pages: {})", victim_idx, min_valid_count);

        // 2. 유효 페이지 대피 (Migration)
        for page_idx in 0..PAGES_PER_BLOCK {
            let is_valid = self.blocks[victim_idx].pages[page_idx].state == PageState::Valid;

            if is_valid {
                let data = self.blocks[victim_idx].pages[page_idx].content;
                let lba_opt = self.find_lba_by_pba(victim_idx as u32, page_idx);

                if let Some(target_lba) = lba_opt {
                    // [핵심 수정] self.write() 대신 self.write_internal() 호출!
                    // 이유: user_write_cnt를 올리면 안 되고, GC 재귀 호출을 막아야 함.
                    
                    if let Err(_) = self.write_internal(target_lba, data) {
                        // 만약 이사 도중에 Active Block이 꽉 찼다면? -> 다음 블록 가져와서 계속 진행
                        if let Some(next_idx) = self.find_next_free_block() {
                            //println!("[GC] Switching block during migration...");
                            self.active_block_idx = next_idx;
                            self.write_internal(target_lba, data).unwrap();
                        } else {
                            panic!("Fatal: No space left during GC migration!");
                        }
                    }
                }
            }
        }

        // 3. 블록 초기화
        self.blocks[victim_idx].erase();
        println!("[GC] Erased Block {}. GC Finished.\n", victim_idx);
    }

    // [보조 함수] 빈 블록 개수 세기 (GC 트리거 조건용)
    fn count_free_blocks(&self) -> usize {
        self.blocks.iter().filter(|b| b.state == BlockState::Free).count()
    }

    fn find_next_free_block(&self) -> Option<usize> {
        for (i, block) in self.blocks.iter().enumerate() {
            // 현재 쓰고 있는 블록은 제외하고 찾기
            if block.state == BlockState::Free && i != self.active_block_idx {
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
}