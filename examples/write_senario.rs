use ssd_rs::block::*;
use ssd_rs::mapping::*;
use ssd_rs::page::*;

fn write(block: &mut Block, data:u32) -> Option<PhysicalAddress> {
    let mut page_offset = 0;
    loop {
        if page_offset >= PAGES_PER_BLOCK {
            return None;
        }
        let cur_page = block.read(page_offset);
        if cur_page.state == PageState::Free {
            block.program(page_offset, data);
            return Some(PhysicalAddress { block_id: block.id, page_offset: page_offset });
        }
        page_offset += 1;
    }
}

fn main() {
    const TABLE_SIZE: usize = 128;
    let mut table = MappingTable::new(TABLE_SIZE);
    // 일단 free block list 관리는 신경쓰지 말 것.
    let mut b = Block::new(0);

    if b.state == BlockState::Full {
        panic!("block is full.")
    }

    let mut data: u32 = 10;
    let pba = write(&mut b, data);
    match pba {
        Some(pba) => {
            if let Some(old_pba) = table.update(0, pba) {
                b.pages[old_pba.page_offset].state = PageState::Invalid;
            }
        },
        None => {
            println!("no block can be written");
        }
    };
    data = 20;
    let pba2 = write(&mut b, data);
    match pba2 {
        Some(pba2) => {
            if let Some(old_pba) = table.update(0, pba2) {
                b.pages[old_pba.page_offset].state = PageState::Invalid;
            }
        },
        None => {
            println!("no block can be written");
        }
    };
    
    // table.update(0, pba);
    println!("{:?}", b);
    println!("{:?}", table);
    
}