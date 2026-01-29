use super::block::*;
use super::mapping::*;
use super::page::*;

pub struct SSD {
    blocks: Vec<Block>,          // 블록들의 아파트 (예: 100개)
    mapping_table: MappingTable, // 주소록 (LBA -> PBA)
    active_block_idx: usize,     // 현재 데이터를 쓰고 있는 '작업 중인 블록'의 인덱스
}

impl SSD {
    // 1. SSD 초기화: 블록 N개와 LBA M개를 생성
    pub fn new(num_blocks: usize, num_lbas: usize) -> Self {
        let mut blocks = Vec::new();
        for i in 0..num_blocks {
            blocks.push(Block::new(i as u32));
        }

        SSD {
            blocks,
            mapping_table: MappingTable::new(num_lbas),
            active_block_idx: 0, // 일단 0번 블록부터 쓰기 시작
        }
    }

    // 2. 쓰기 요청 처리 (사용자가 작성한 로직 통합)
    pub fn write(&mut self, lba: usize, data: u32) -> Result<(), String> {
        // (1) 현재 Active Block이 꽉 찼는지 확인
        if self.blocks[self.active_block_idx].state == BlockState::Full {
            // 꽉 찼으면 '다음 빈 블록'을 찾아야 함 (Allocate Free Block)
            match self.find_next_free_block() {
                Some(idx) => {
                    //println!("Block {} is full. Switching to Block {}...", self.active_block_idx, idx);
                    self.active_block_idx = idx;
                },
                None => {
                    // 빈 블록도 없다면? -> 여기서 나중에 [GC]가 호출되어야 함!
                    self.gc(); // 청소 시작 (희생자 찾아서 데이터 옮기고 Erase)

                    if let Some(idx) = self.find_next_free_block() {
                        self.active_block_idx = idx;
                        //println!(">>> GC Finished. Switched to Block {}", idx);
                    } else {
                        // GC를 했는데도 빈 블록이 없다? (SSD가 진짜 꽉 참)
                        return Err("Fatal Error: SSD is Full even after GC!".to_string());
                    }
                }
            }
        }

        // (2) Active Block 가져오기
        let block = &mut self.blocks[self.active_block_idx];

        // (3) 빈 페이지 찾아서 쓰기 (작성하신 로직)
        let mut target_page = None;
        for page_offset in 0..PAGES_PER_BLOCK {
            if block.read(page_offset).state == PageState::Free {
                block.program(page_offset, data);
                target_page = Some(page_offset);
                break;
            }
        }

        // (4) 매핑 테이블 갱신 및 Old Data 무효화 (작성하신 로직)
        if let Some(page_offset) = target_page {
            let new_pba = PhysicalAddress {
                block_id: block.id,
                page_offset,
            };

            // [중요] 예전 데이터 위치를 받아서 Invalidate 처리
            if let Some(old_pba) = self.mapping_table.update(lba, new_pba) {
                // 예전 블록의 페이지를 쓰레기(Invalid)로 만듦
                let old_blk_idx = old_pba.block_id as usize;
                self.blocks[old_blk_idx].pages[old_pba.page_offset].state = PageState::Invalid;
                println!("  -> Invalidated Old Data: Block {} Page {}", old_blk_idx, old_pba.page_offset);
            }
            Ok(())
        } else {
            // 이론상 여기 도달하면 안 됨 (위에서 Full 체크를 했으므로)
            Err("Unexpected Error: Active block has no space.".to_string())
        }
    }

    // 헬퍼 함수: 다음 빈 블록 찾기 (순차 탐색)
    fn find_next_free_block(&self) -> Option<usize> {
        for (i, block) in self.blocks.iter().enumerate() {
            if block.state == BlockState::Free && i != self.active_block_idx {
                return Some(i);
            }
        }
        None
    }

    pub fn print_blocks(&self) {
        for block in &self.blocks {
            println!("{:?}", block);
        }
        println!("===============================")
    }

    
    pub fn gc(&mut self) {
        //println!("\n Garbage Collection Started!");

        let mut victim_idx = None;
        let mut min_valid_count = usize::MAX;

        for (i, block) in self.blocks.iter().enumerate() {
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
                println!("Failed to find victim block!");
                return;
            }
        };

        for page_idx in 0..PAGES_PER_BLOCK {
            let is_valid = self.blocks[victim_idx].pages[page_idx].state == PageState::Valid;

            if is_valid {
                let data = self.blocks[victim_idx].pages[page_idx].content;
                let lba = self.find_lba_by_pba(victim_idx as u32, page_idx);

                if let Some(target_lba) = lba {
                    self.write(target_lba, data).unwrap();
                }

            }
        }

        self.blocks[victim_idx].erase();

    }

    fn find_lba_by_pba(&self, block_id: u32, page_offset: usize) -> Option<usize> {
        for (lba, entry) in self.mapping_table.entries().iter().enumerate() {
            if let Some(pba) = entry {
                if pba.block_id == block_id && pba.page_offset == page_offset {
                    return Some(lba);
                }
            }
        }
        None
    }
}