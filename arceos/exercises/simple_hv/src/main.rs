#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]
#![feature(asm_const)]
#![feature(riscv_ext_intrinsics)]

#[cfg(feature = "axstd")]
extern crate axstd as std;
extern crate alloc;
#[macro_use]
extern crate axlog;

mod task;
mod vcpu;
mod regs;
mod csrs;
mod sbi;
mod loader;

use vcpu::VmCpuRegisters;
use riscv::register::{scause, sstatus, stval};
use csrs::defs::hstatus;
use tock_registers::LocalRegisterCopy;
use csrs::{RiscvCsrTrait, CSR};
use vcpu::_run_guest;
use sbi::SbiMessage;
use loader::load_vm_image;
use axhal::mem::{PAGE_SIZE_4K, PhysAddr, VirtAddr};
use axhal::paging::MappingFlags;
use crate::regs::{GprIndex, GprIndex::{A0, A1}};

const VM_ENTRY: usize = 0x8020_0000;
/// The boot parameter read by the guest from physical address 0x40.
const VM_BOOT_PARAM: usize = 0x6688;
/// The hart id returned when the guest reads `mhartid`.
const VM_HART_ID: usize = 0x1234;

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    ax_println!("Hypervisor ...");

    // A new address space for vm.
    let mut uspace = axmm::new_user_aspace().unwrap();

    // Load vm binary file into address space.
    if let Err(e) = load_vm_image("/sbin/skernel2", &mut uspace) {
        panic!("Cannot load app! {:?}", e);
    }

    // Map the page at guest physical address 0, where the guest reads its
    // boot parameter from address 0x40 (see payload/skernel2).
    uspace
        .map_alloc(
            VirtAddr::from(0usize),
            PAGE_SIZE_4K,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
            true,
        )
        .unwrap();
    uspace
        .write(VirtAddr::from(0x40), &VM_BOOT_PARAM.to_le_bytes())
        .unwrap();

    // Setup context to prepare to enter guest mode.
    let mut ctx = VmCpuRegisters::default();
    prepare_guest_context(&mut ctx);

    // Setup pagetable for 2nd address mapping.
    let ept_root = uspace.page_table_root();
    prepare_vm_pgtable(ept_root);

    // Disable the HS-level timer interrupt while running the guest: HS-level
    // interrupts are always enabled in VS-mode, so a pending STIP would kick
    // us out of the guest before it executes a single instruction.
    CSR.sie.read_and_clear_bits(1 << 5);

    // Kick off vm and wait for it to exit.
    while !run_guest(&mut ctx) {
    }

    panic!("Hypervisor ok!");
}

fn prepare_vm_pgtable(ept_root: PhysAddr) {
    let hgatp = 8usize << 60 | usize::from(ept_root) >> 12;
    unsafe {
        core::arch::asm!(
            "csrw hgatp, {hgatp}",
            hgatp = in(reg) hgatp,
        );
        core::arch::riscv64::hfence_gvma_all();
    }
}

fn run_guest(ctx: &mut VmCpuRegisters) -> bool {
    unsafe {
        _run_guest(ctx);
    }

    vmexit_handler(ctx)
}

#[allow(unreachable_code)]
fn vmexit_handler(ctx: &mut VmCpuRegisters) -> bool {
    use scause::{Exception, Trap};

    let scause = scause::read();
    match scause.cause() {
        Trap::Exception(Exception::VirtualSupervisorEnvCall) => {
            let sbi_msg = SbiMessage::from_regs(ctx.guest_regs.gprs.a_regs()).ok();
            ax_println!("VmExit Reason: VSuperEcall: {:?}", sbi_msg);
            if let Some(msg) = sbi_msg {
                match msg {
                    SbiMessage::Reset(_) => {
                        let a0 = ctx.guest_regs.gprs.reg(A0);
                        let a1 = ctx.guest_regs.gprs.reg(A1);
                        ax_println!("a0 = {:#x}, a1 = {:#x}", a0, a1);
                        assert_eq!(a0, 0x6688);
                        assert_eq!(a1, 0x1234);
                        ax_println!("Shutdown vm normally!");
                        return true;
                    },
                    _ => todo!(),
                }
            } else {
                panic!("bad sbi message! ");
            }
        },
        Trap::Exception(Exception::IllegalInstruction) => {
            // The guest is allowed to read M-mode CSRs such as `mhartid`;
            // QEMU reports the faulting instruction in stval. Emulate the CSR
            // read and resume the guest.
            let insn = stval::read();
            let csr = (insn >> 20) & 0xfff;
            if insn != 0 && (insn & 0x3) == 0x3 && csr == 0xF14 {
                // mhartid read: return the emulated hart id.
                ax_println!("VmExit Reason: IllegalInstruction: mhartid read");
                let rd = ((insn >> 7) & 0x1f) as u32;
                if let Some(gpr) = GprIndex::from_raw(rd) {
                    ctx.guest_regs.gprs.set_reg(gpr, VM_HART_ID);
                }
                ctx.guest_regs.sepc += 4;
                return false;
            }
            panic!("Bad instruction: {:#x} sepc: {:#x}",
                stval::read(),
                ctx.guest_regs.sepc
            );
        },
        Trap::Exception(Exception::LoadGuestPageFault) => {
            panic!("LoadGuestPageFault: stval{:#x} sepc: {:#x}",
                stval::read(),
                ctx.guest_regs.sepc
            );
        },
        _ => {
            panic!(
                "Unhandled trap: {:?}, sepc: {:#x}, stval: {:#x}",
                scause.cause(),
                ctx.guest_regs.sepc,
                stval::read()
            );
        }
    }
    false
}

fn prepare_guest_context(ctx: &mut VmCpuRegisters) {
    // Set hstatus
    let mut hstatus = LocalRegisterCopy::<usize, hstatus::Register>::new(
        riscv::register::hstatus::read().bits(),
    );
    // Set Guest bit in order to return to guest mode.
    hstatus.modify(hstatus::spv::Guest);
    // SPVP must be User: the HS->VS `sret` sets VS-mode's SIE from
    // hstatus.SPVP, so leaving it Supervisor would enable interrupts in the
    // guest immediately at entry.
    hstatus.modify(hstatus::spvp::User);
    CSR.hstatus.write_value(hstatus.get());
    ctx.guest_regs.hstatus = hstatus.get();

    // Set sstatus in guest mode.
    let mut sstatus = sstatus::read();
    sstatus.set_spp(sstatus::SPP::Supervisor);
    // Clear SIE and SPIE: `sret` sets SIE from SPIE, so a pending timer
    // interrupt would be delivered to the guest immediately on entry while
    // stvec still points at the hypervisor exit path.
    ctx.guest_regs.sstatus = sstatus.bits() & !((1 << 1) | (1 << 5));
    // Return to entry to start vm.
    ctx.guest_regs.sepc = VM_ENTRY;
}
