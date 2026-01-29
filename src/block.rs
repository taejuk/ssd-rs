use std::fmt;
use super::page::{Page, PageState};

pub const PAGES_PER_BLOCK: usize = 64;

#[derive(Debug)]
pub struct WearStats {
    pub min: u32,
    pub max: u32,
    pub avg: f64,
    pub gap: u32, 
}


#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum BlockState {
    Free,
    Active,
    Full,
}

#[derive(Clone)]
pub struct Block {
    pub id: u32,
    pub pages: Vec<Page>,
    pub erase_count: u32,
    pub is_bad: bool,
    pub state: BlockState,
}

impl Block {
    pub fn new(id: u32) -> Self {
        let mut pages = Vec::with_capacity(PAGES_PER_BLOCK);
        for _ in 0..PAGES_PER_BLOCK {
            pages.push(Page {
                content: 0,
                state: PageState::Free,
            });
        }

        Block {
            id,
            pages,
            erase_count: 0,
            is_bad: false,
            state: BlockState::Free,
        }
    }

    // 2. 읽기 (Read): 특정 오프셋의 페이지를 읽음
    pub fn read(&self, page_offset: usize) -> &Page {
        if page_offset >= PAGES_PER_BLOCK {
            panic!("Block {}: Page offset {} is out of bounds!", self.id, page_offset);
        }
        &self.pages[page_offset]
    }

    // 3. 쓰기 (Program): 낸드 플래시의 제약을 강제함 (덮어쓰기 금지!)
    pub fn program(&mut self, page_offset: usize, data: u32) {
        if self.is_bad {
            panic!("Block {}: Cannot write to a BAD block!", self.id);
        }
        if page_offset >= PAGES_PER_BLOCK {
            panic!("Block {}: Page offset {} out of bounds", self.id, page_offset);
        }

        let page = &mut self.pages[page_offset];

        // [Constraint] 이미 데이터가 있는 곳(Valid/Invalid)에는 쓸 수 없다!
        if page.state != PageState::Free {
            panic!(
                "Block {} Page {}: Cannot overwrite! Must erase block first. (State: {:?})",
                self.id, page_offset, page.state
            );
        }

        // 데이터 쓰기 및 상태 변경
        page.content = data;
        page.state = PageState::Valid;

        // 블록 상태 업데이트 (Free -> Active)
        if self.state == BlockState::Free {
            self.state = BlockState::Active;
        }

        // 만약 마지막 페이지까지 다 썼다면 Full로 변경
        if page_offset == PAGES_PER_BLOCK - 1 {
            self.state = BlockState::Full;
        }
    }

    pub fn erase(&mut self) {
        if self.is_bad {
            println!("Warning: Attempting to erase Bad Block {}", self.id);
            return;
        }

        self.erase_count += 1;
        
        self.state = BlockState::Free;

        for page in self.pages.iter_mut() {
            page.content = 0;
            page.state = PageState::Free;
        }
    }
    
    pub fn count_valid_pages(&self) -> usize {
        self.pages.iter().filter(|p| p.state == PageState::Valid).count()
    }
}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Physical Block #{} ===", self.id)?;
        writeln!(f, "  State:      {:?}", self.state)?;
        writeln!(f, "  Erase Cnt:  {}", self.erase_count)?;
        writeln!(f, "  Valid Pgs:  {}/{}", self.count_valid_pages(), PAGES_PER_BLOCK)?;
        writeln!(f, "  Is Bad:     {}", self.is_bad)?;
        write!(f, "  Map: [")?;

        for (i, page) in self.pages.iter().enumerate() {
            if i % 16 == 0 && i != 0 {
                write!(f, "\n        ")?;
            }
            let symbol = match page.state {
                PageState::Valid => "V",
                PageState::Invalid => "I",
                PageState::Free => ".",
            };
            write!(f, "{}", symbol)?;
        }
        write!(f, "]")
    }
}