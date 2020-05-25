// @TODO: temporal
const TEST_MEMORY_CAPACITY: u64 = 1024 * 512;
const PROGRAM_MEMORY_CAPACITY: u64 = 1024 * 1024 * 128; // big enough to run Linux and xv6

pub mod cpu;
pub mod terminal;
pub mod default_terminal;
pub mod memory;
pub mod mmu;
pub mod device;

use cpu::{Cpu, Xlen};
use terminal::Terminal;

/// RISC-V emulator. It emulates RISC-V CPU and peripheral devices.
///
/// Sample code to run the emulator.
/// ```ignore
/// // Creates an emulator with arbitary terminal
/// let mut emulator = Emulator::new(Box::new(DefaultTerminal::new()));
/// // Set up program content binary
/// emulator.setup_program(program_content);
/// // Set up Filesystem content binary
/// emulator.setup_filesystem(fs_content);
/// // Go!
/// emulator.run();
/// ```
pub struct Emulator {
	cpu: Cpu,

	/// [`riscv-tests`](https://github.com/riscv/riscv-tests) program specific
	/// properties. Whether the program set by `setup_program()` is
	/// [`riscv-tests`](https://github.com/riscv/riscv-tests) program.
	is_test: bool,

	/// [`riscv-tests`](https://github.com/riscv/riscv-tests) specific properties.
	/// The address where data will be sent to terminal
	tohost_addr: u64
}

/// ELF section header
struct SectionHeader {
	sh_name: u64,
	_sh_type: u64,
	_sh_flags: u64,
	sh_addr: u64,
	sh_offset: u64,
	sh_size: u64,
	_sh_link: u64,
	_sh_info: u64,
	_sh_addralign: u64,
	_sh_entsize: u64
}

impl Emulator {
	/// Creates a new `Emulator`. [`Terminal`](terminal/trait.Terminal.html)
	/// is internally used for transferring input/output data to/from `Emulator`.
	/// 
	/// # Arguments
	/// * `terminal`
	pub fn new(terminal: Box<dyn Terminal>) -> Self {
		Emulator {
			cpu: Cpu::new(terminal),

			// These can be updated in setup_program()
			is_test: false,
			tohost_addr: 0
		}
	}

	/// Runs program set by `setup_program()`. Calls `run_test()` if the program
	/// is [`riscv-tests`](https://github.com/riscv/riscv-tests).
	/// Otherwise calls `run_program()`.
	pub fn run(&mut self) {
		match self.is_test {
			true => self.run_test(),
			false => self.run_program()
		};
	}

	/// Runs program set by `setup_program()`. The emulator won't stop forever.
	pub fn run_program(&mut self) {
		loop {
			self.tick();
		}
	}

	/// Method for running [`riscv-tests`](https://github.com/riscv/riscv-tests) program.
	/// The differences from `run_program()` are
	/// * Disassembles every instruction and dumps to terminal
	/// * The emulator stops when the test finishes
	/// * Displays the result message (pass/fail) to terminal
	pub fn run_test(&mut self) {
		// @TODO: Send this message to terminal?
		println!("This elf file seems riscv-tests elf file. Running in test mode.");
		loop {
			let disas = self.cpu.disassemble_next_instruction();
			self.put_bytes_to_terminal(disas.as_bytes());
			self.put_bytes_to_terminal(&[10]); // new line

			self.tick();

			// It seems in riscv-tests ends with end code
			// written to a certain physical memory address
			// (0x80001000 in mose test cases) so checking
			// the data in the address and terminating the test
			// if non-zero data is written.
			// End code 1 seems to mean pass.
			let endcode = self.cpu.get_mut_mmu().load_word_raw(self.tohost_addr);
			if endcode != 0 {
				match endcode {
					1 => {
						self.put_bytes_to_terminal(format!("Test Passed with {:X}\n", endcode).as_bytes())
					},
					_ => {
						self.put_bytes_to_terminal(format!("Test Failed with {:X}\n", endcode).as_bytes())
					}
				};
				break;
			}
		}
	}

	/// Helper method. Sends ascii code bytes to terminal.
	///
	/// # Arguments
	/// * `bytes`
	fn put_bytes_to_terminal(&mut self, bytes: &[u8]) {
		for i in 0..bytes.len() {
			self.cpu.get_mut_terminal().put_byte(bytes[i]);
		}
	}

	/// Runs CPU one cycle
	pub fn tick(&mut self) {
		self.cpu.tick();
	}

	/// Sets up program run by the program. This method analyzes the passed content
	/// and configure CPU properly. If the passed contend doesn't seem ELF file,
	/// it panics. This method is expected to be called only once.
	///
	/// # Arguments
	/// * `data` Program binary
	// @TODO: Make ElfAnalyzer and move the core logic there.
	// @TODO: Returns `Err` if the passed contend doesn't seem ELF file
	pub fn setup_program(&mut self, data: Vec<u8>) {
		// analyze elf header

		// check ELF magic number
		if data[0] != 0x7f || data[1] != 0x45 || data[2] != 0x4c || data[3] != 0x46 {
			panic!("This file does not seem ELF file");
		}

		let e_class = data[4];

		let e_width = match e_class {
			1 => 32,
			2 => 64,
			_ => panic!("Unknown e_class:{:X}", e_class)
		};

		let _e_endian = data[5];
		let _e_elf_version = data[6];
		let _e_osabi = data[7];
		let _e_abi_version = data[8];

		let mut offset = 0x10;

		let mut _e_type = 0 as u64;
		for i in 0..2 {
			_e_type |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_machine = 0 as u64;
		for i in 0..2 {
			_e_machine |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_version = 0 as u64;
		for i in 0..4 {
			_e_version |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut e_entry = 0 as u64;
		for i in 0..e_width / 8 {
			e_entry |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_phoff = 0 as u64;
		for i in 0..e_width / 8 {
			_e_phoff |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut e_shoff = 0 as u64;
		for i in 0..e_width / 8 {
			e_shoff |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_flags = 0 as u64;
		for i in 0..4 {
			_e_flags |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_ehsize = 0 as u64;
		for i in 0..2 {
			_e_ehsize |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_phentsize = 0 as u64;
		for i in 0..2 {
			_e_phentsize |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_phnum = 0 as u64;
		for i in 0..2 {
			_e_phnum |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_shentsize = 0 as u64;
		for i in 0..2 {
			_e_shentsize |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut e_shnum = 0 as u64;
		for i in 0..2 {
			e_shnum |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		let mut _e_shstrndx = 0 as u64;
		for i in 0..2 {
			_e_shstrndx |= (data[offset] as u64) << (8 * i);
			offset += 1;
		}

		/*
		println!("ELF:{}", e_width);
		println!("e_endian:{:X}", _e_endian);
		println!("e_elf_version:{:X}", _e_elf_version);
		println!("e_osabi:{:X}", _e_osabi);
		println!("e_abi_version:{:X}", _e_abi_version);
		println!("e_type:{:X}", _e_type);
		println!("e_machine:{:X}", _e_machine);
		println!("e_version:{:X}", _e_version);
		println!("e_entry:{:X}", e_entry);
		println!("e_phoff:{:X}", _e_phoff);
		println!("e_shoff:{:X}", e_shoff);
		println!("e_flags:{:X}", _e_flags);
		println!("e_ehsize:{:X}", _e_ehsize);
		println!("e_phentsize:{:X}", _e_phentsize);
		println!("e_phnum:{:X}", _e_phnum);
		println!("e_shentsize:{:X}", _e_shentsize);
		println!("e_shnum:{:X}", e_shnum);
		println!("e_shstrndx:{:X}", _e_shstrndx);
		*/

		// analyze program headers

		/*
		offset = e_phoff as usize;
		for i in 0..e_phnum {
			let mut p_type = 0 as u64;
			for i in 0..4 {
				p_type |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut p_flags = 0 as u64;
			if e_width == 64 {
				for i in 0..4 {
					p_flags |= (data[offset] as u64) << (8 * i);
					offset += 1;
				}
			}

			let mut p_offset = 0 as u64;
			for i in 0..e_width / 8 {
				p_offset |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut p_vaddr = 0 as u64;
			for i in 0..e_width / 8 {
				p_vaddr |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut p_paddr = 0 as u64;
			for i in 0..e_width / 8 {
				p_paddr |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut p_filesz = 0 as u64;
			for i in 0..e_width / 8 {
				p_filesz |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut p_memsz = 0 as u64;
			for i in 0..e_width / 8 {
				p_memsz |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			if e_width == 32 {
				for i in 0..4 {
					p_flags |= (data[offset] as u64) << (8 * i);
					offset += 1;
				}
			}

			let mut p_align = 0 as u64;
			for i in 0..e_width / 8 {
				p_align |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			println!("");
			println!("Program:{:X}", i);
			println!("p_type:{:X}", p_type);
			println!("p_flags:{:X}", p_flags);
			println!("p_offset:{:X}", p_offset);
			println!("p_vaddr:{:X}", p_vaddr);
			println!("p_paddr:{:X}", p_paddr);
			println!("p_filesz:{:X}", p_filesz);
			println!("p_memsz:{:X}", p_memsz);
			println!("p_align:{:X}", p_align);
			println!("p_align:{:X}", p_align);
		}
		*/

		// analyze section headers

		let mut program_data_section_headers = vec![];
		let mut string_table_section_headers = vec![];

		offset = e_shoff as usize;
		for _i in 0..e_shnum {
			let mut sh_name = 0 as u64;
			for i in 0..4 {
				sh_name |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_type = 0 as u64;
			for i in 0..4 {
				sh_type |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_flags = 0 as u64;
			for i in 0..e_width / 8 {
				sh_flags |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_addr = 0 as u64;
			for i in 0..e_width / 8 {
				sh_addr |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_offset = 0 as u64;
			for i in 0..e_width / 8 {
				sh_offset |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_size = 0 as u64;
			for i in 0..e_width / 8 {
				sh_size |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_link = 0 as u64;
			for i in 0..4 {
				sh_link |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_info = 0 as u64;
			for i in 0..4 {
				sh_info |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_addralign = 0 as u64;
			for i in 0..e_width / 8 {
				sh_addralign |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			let mut sh_entsize = 0 as u64;
			for i in 0..e_width / 8 {
				sh_entsize |= (data[offset] as u64) << (8 * i);
				offset += 1;
			}

			/*
			println!("");
			println!("Section:{:X}", i);
			println!("sh_name:{:X}", sh_name);
			println!("sh_type:{:X}", sh_type);
			println!("sh_flags:{:X}", sh_flags);
			println!("sh_addr:{:X}", sh_addr);
			println!("sh_offset:{:X}", sh_offset);
			println!("sh_size:{:X}", sh_size);
			println!("sh_link:{:X}", sh_link);
			println!("sh_info:{:X}", sh_info);
			println!("sh_addralign:{:X}", sh_addralign);
			println!("sh_entsize:{:X}", sh_entsize);
			*/

			let section_header = SectionHeader {
				sh_name: sh_name,
				_sh_type: sh_type,
				_sh_flags: sh_flags,
				sh_addr: sh_addr,
				sh_offset: sh_offset,
				sh_size: sh_size,
				_sh_link: sh_link,
				_sh_info: sh_info,
				_sh_addralign: sh_addralign,
				_sh_entsize: sh_entsize
			};

			if sh_type == 1 {
				program_data_section_headers.push(section_header);
			} else if sh_type == 3 {
				string_table_section_headers.push(section_header);
			}
		}

		// Find program data section named .tohost to detect if the elf file is riscv-tests
		// @TODO: Expecting it can be only in the first string table section.
		// What if .tohost section name is in the second or later string table sectioin?
		let tohost_values = vec![0x2e, 0x74, 0x6f, 0x68, 0x6f, 0x73, 0x74, 0x00]; // ".tohost\null"
		let mut tohost_addr = 0; // Expecting .tohost address is non-null if exists
		for i in 0..program_data_section_headers.len() {
			let sh_addr = program_data_section_headers[i].sh_addr;
			let sh_name = program_data_section_headers[i].sh_name;
			for j in 0..string_table_section_headers.len() {
				let sh_offset = string_table_section_headers[j].sh_offset;
				let sh_size = string_table_section_headers[j].sh_size;
				let mut found = true;
				for k in 0..tohost_values.len() as u64{
					let addr = sh_offset + sh_name + k;
					if addr >= sh_offset + sh_size || data[addr as usize] != tohost_values[k as usize] {
						found = false;
						break;
					}
				}
				if found {
					tohost_addr = sh_addr;
				}
			}
			if tohost_addr != 0 {
				break;
			}
		}

		// Detected whether the elf file is riscv-tests.
		// Setting up CPU and Memory depending on it.

		self.cpu.update_xlen(match e_width {
			32 => Xlen::Bit32,
			64 => Xlen::Bit64,
			_ => panic!("No happen")
		});

		if tohost_addr != 0 {
			self.is_test = true;
			self.tohost_addr = tohost_addr;
			self.cpu.get_mut_mmu().init_memory(TEST_MEMORY_CAPACITY);
		} else {
			self.is_test = false;
			self.tohost_addr = 0;
			self.cpu.get_mut_mmu().init_memory(PROGRAM_MEMORY_CAPACITY);
		}

		for i in 0..program_data_section_headers.len() {
			let sh_addr = program_data_section_headers[i].sh_addr;
			let sh_offset = program_data_section_headers[i].sh_offset;
			let sh_size = program_data_section_headers[i].sh_size;
			if sh_addr >= 0x80000000 && sh_offset > 0 && sh_size > 0 {
				for j in 0..sh_size as usize {
					self.cpu.get_mut_mmu().store_raw(sh_addr + j as u64, data[sh_offset as usize + j]);
				}
			}
		}

		self.cpu.update_pc(e_entry);
	}

	/// Sets up filesystem. Use this method if program (e.g. Linux) uses
	/// filesystem. This method is expected to be called up to only once.
	///
	/// # Arguments
	/// * `content` File system content binary
	pub fn setup_filesystem(&mut self, content: Vec<u8>) {
		self.cpu.get_mut_mmu().init_disk(content);
	}

	/// Sets up device tree. The emulator has default device tree configuration.
	/// If you want to override it, use this method. This method is expected to
	/// to be called up to only once.
	///
	/// # Arguments
	/// * `content` DTB content binary
	pub fn setup_dtb(&mut self, content: Vec<u8>) {
		self.cpu.get_mut_mmu().init_dtb(content);
	}

	/// Updates XLEN (the width of an integer register in bits) in CPU.
	///
	/// # Arguments
	/// * `xlen`
	pub fn update_xlen(&mut self, xlen: Xlen) {
		self.cpu.update_xlen(xlen);
	}

	/// Returns mutable reference to `Terminal`.
	pub fn get_mut_terminal(&mut self) -> &mut Box<dyn Terminal> {
		self.cpu.get_mut_terminal()
	}

	/// Returns immutable reference to `Cpu`.
	pub fn get_cpu(&self) -> &Cpu {
		&self.cpu
	}

	/// Returns mutable reference to `Cpu`.
	pub fn get_mut_cpu(&mut self) -> &mut Cpu {
		&mut self.cpu
	}
}