/* x86_64 memory management — adapted from i386 with 4-level paging:
 * - 512 entries per page table level (vs 1024 on i386)
 * - 2MB large pages (vs 4MB on i386)
 * - 64-bit PTE entries
 */

#include "kernel/kernel.h"
#include "kernel/vm.h"

#include <machine/vm.h>

#include <minix/syslib.h>
#include <minix/cpufeature.h>
#include <string.h>
#include <assert.h>
#include <signal.h>
#include <stdlib.h>

#include <machine/vm.h>

#include "oxpcie.h"
#include "arch_proto.h"

#ifdef USE_APIC
#include "apic.h"
#ifdef USE_WATCHDOG
#include "kernel/watchdog.h"
#endif
#endif

phys_bytes video_mem_vaddr = 0;

#define HASPT(procptr) ((procptr)->p_seg.p_cr3 != 0)
static int nfreepdes = 0;
#define MAXFREEPDES	2
static int freepdes[MAXFREEPDES];

static u64_t phys_get64(phys_bytes v);

void mem_clear_mapcache(void)
{
	int i;
	for(i = 0; i < nfreepdes; i++) {
		struct proc *ptproc = get_cpulocal_var(ptproc);
		int pde = freepdes[i];
		u64_t *ptv;
		assert(ptproc);
		ptv = (u64_t *)ptproc->p_seg.p_cr3_v;
		assert(ptv);
		ptv[pde] = 0;
	}
}

static phys_bytes createpde(
	const struct proc *pr,
	const phys_bytes linaddr,
	phys_bytes *bytes,
	int free_pde_idx,
	int *changed
	)
{
	u64_t pdeval;
	phys_bytes offset;
	int pde;

	assert(free_pde_idx >= 0 && free_pde_idx < nfreepdes);
	pde = freepdes[free_pde_idx];
	assert(pde >= 0 && pde < X86_64_VM_DIR_ENTRIES);

	if(pr && ((pr == get_cpulocal_var(ptproc)) || iskernelp(pr))) {
		return linaddr;
	}

	if(pr) {
		u64_t *cr3_v = (u64_t *)pr->p_seg.p_cr3_v;
		assert(cr3_v);
		pdeval = cr3_v[X86_64_VM_PDE(linaddr)];
	} else {
		pdeval = (linaddr & X86_64_VM_ADDR_MASK_2MB) |
			X86_64_VM_BIGPAGE | X86_64_VM_PRESENT |
			X86_64_VM_WRITE | X86_64_VM_USER;
	}

	u64_t *ptcr3_v = (u64_t *)get_cpulocal_var(ptproc)->p_seg.p_cr3_v;
	assert(ptcr3_v);
	if(ptcr3_v[pde] != pdeval) {
		ptcr3_v[pde] = pdeval;
		*changed = 1;
	}

	offset = linaddr & X86_64_VM_OFFSET_MASK_2MB;
	*bytes = MIN(*bytes, X86_64_BIG_PAGE_SIZE - offset);

	return X86_64_BIG_PAGE_SIZE * pde + offset;
}

static int check_resumed_caller(struct proc *caller)
{
	if (caller && (caller->p_misc_flags & MF_KCALL_RESUME)) {
		assert(caller->p_vmrequest.vmresult != VMSUSPEND);
		return caller->p_vmrequest.vmresult;
	}
	return OK;
}

static int lin_lin_copy(struct proc *srcproc, vir_bytes srclinaddr,
	struct proc *dstproc, vir_bytes dstlinaddr, vir_bytes bytes)
{
	u64_t addr;
	proc_nr_t procslot;

	assert(get_cpulocal_var(ptproc));
	assert(get_cpulocal_var(proc_ptr));
	assert(read_cr3() == get_cpulocal_var(ptproc)->p_seg.p_cr3);

	procslot = get_cpulocal_var(ptproc)->p_nr;

	assert(procslot >= 0 && procslot < X86_64_VM_DIR_ENTRIES);

	if(srcproc) assert(!RTS_ISSET(srcproc, RTS_SLOT_FREE));
	if(dstproc) assert(!RTS_ISSET(dstproc, RTS_SLOT_FREE));
	assert(!RTS_ISSET(get_cpulocal_var(ptproc), RTS_SLOT_FREE));
	assert(get_cpulocal_var(ptproc)->p_seg.p_cr3_v);
	if(srcproc) assert(!RTS_ISSET(srcproc, RTS_VMINHIBIT));
	if(dstproc) assert(!RTS_ISSET(dstproc, RTS_VMINHIBIT));

	while(bytes > 0) {
		phys_bytes srcptr, dstptr;
		vir_bytes chunk = bytes;
		int changed = 0;

#ifdef CONFIG_SMP
		unsigned cpu = cpuid;
		if (srcproc && GET_BIT(srcproc->p_stale_tlb, cpu)) {
			changed = 1;
			UNSET_BIT(srcproc->p_stale_tlb, cpu);
		}
		if (dstproc && GET_BIT(dstproc->p_stale_tlb, cpu)) {
			changed = 1;
			UNSET_BIT(dstproc->p_stale_tlb, cpu);
		}
#endif

		srcptr = createpde(srcproc, srclinaddr, &chunk, 0, &changed);
		dstptr = createpde(dstproc, dstlinaddr, &chunk, 1, &changed);
		if(changed)
			reload_cr3();

		if (srcptr + chunk < srcptr) return EFAULT_SRC;
		if (dstptr + chunk < dstptr) return EFAULT_DST;

		PHYS_COPY_CATCH(srcptr, dstptr, chunk, addr);

		if(addr) {
			if(addr >= srcptr && addr < (srcptr + chunk)) {
				return EFAULT_SRC;
			}
			if(addr >= dstptr && addr < (dstptr + chunk)) {
				return EFAULT_DST;
			}
			panic("lin_lin_copy fault out of range");
			return EFAULT;
		}

		bytes -= chunk;
		srclinaddr += chunk;
		dstlinaddr += chunk;
	}

	if(srcproc) assert(!RTS_ISSET(srcproc, RTS_SLOT_FREE));
	if(dstproc) assert(!RTS_ISSET(dstproc, RTS_SLOT_FREE));
	assert(!RTS_ISSET(get_cpulocal_var(ptproc), RTS_SLOT_FREE));
	assert(get_cpulocal_var(ptproc)->p_seg.p_cr3_v);

	return OK;
}

static u64_t phys_get64(phys_bytes addr)
{
	u64_t v;
	int r;

	if((r=lin_lin_copy(NULL, addr,
		proc_addr(SYSTEM), (phys_bytes) &v, sizeof(v))) != OK) {
		panic("lin_lin_copy for phys_get64 failed: %d", r);
	}
	return v;
}

phys_bytes umap_virtual(
  register struct proc *rp,
  int seg,
  vir_bytes vir_addr,
  vir_bytes bytes
)
{
	phys_bytes phys = 0;

	if(vm_lookup(rp, vir_addr, &phys, NULL) != OK) {
		printf("SYSTEM:umap_virtual: vm_lookup of %s: seg 0x%x: 0x%lx failed\n",
			rp->p_name, seg, vir_addr);
		phys = 0;
	} else {
		if(phys == 0)
			panic("vm_lookup returned phys: 0x%lx", phys);
	}

	if(phys == 0) {
		printf("SYSTEM:umap_virtual: lookup failed\n");
		return 0;
	}

	if(bytes > 0 && vm_lookup_range(rp, vir_addr, NULL, bytes) != bytes) {
		printf("umap_virtual: %s: %lu at 0x%lx (vir 0x%lx) not contiguous\n",
			rp->p_name, bytes, vir_addr, vir_addr);
		return 0;
	}

	assert(phys);
	return phys;
}

int vm_lookup(const struct proc *proc, const vir_bytes virtual,
 phys_bytes *physical, u32_t *ptent)
{
	u64_t *root, *pt;
	int pde, pte;
	u64_t pde_v, pte_v;

	assert(proc);
	assert(physical);
	assert(!isemptyp(proc));
	assert(HASPT(proc));

	root = (u64_t *) proc->p_seg.p_cr3;
	assert(!((u64_t) root % X86_64_PAGE_SIZE));
	pde = X86_64_VM_PDE(virtual);
	assert(pde >= 0 && pde < X86_64_VM_DIR_ENTRIES);
	pde_v = phys_get64((phys_bytes) (root + pde));

	if(!(pde_v & X86_64_VM_PRESENT)) {
		return EFAULT;
	}

	if(pde_v & X86_64_VM_BIGPAGE) {
		*physical = (phys_bytes)(pde_v & X86_64_VM_ADDR_MASK_2MB);
		if(ptent) *ptent = (u32_t)pde_v;
		*physical += virtual & X86_64_VM_OFFSET_MASK_2MB;
	} else {
		pt = (u64_t *) X86_64_VM_PFA(pde_v);
		assert(!((u64_t) pt % X86_64_PAGE_SIZE));
		pte = X86_64_VM_PTE(virtual);
		assert(pte >= 0 && pte < X86_64_VM_PT_ENTRIES);
		pte_v = phys_get64((phys_bytes) (pt + pte));
		if(!(pte_v & X86_64_VM_PRESENT)) {
			return EFAULT;
		}

		if(ptent) *ptent = (u32_t)pte_v;

		*physical = X86_64_VM_PFA(pte_v);
		*physical += virtual % X86_64_PAGE_SIZE;
	}

	return OK;
}

size_t vm_lookup_range(const struct proc *proc, vir_bytes vir_addr,
	phys_bytes *phys_addr, size_t bytes)
{
	phys_bytes phys, next_phys;
	size_t len;

	assert(proc);
	assert(bytes > 0);
	assert(HASPT(proc));

	if (vm_lookup(proc, vir_addr, &phys, NULL) != OK)
		return 0;

	if (phys_addr != NULL)
		*phys_addr = phys;

	len = X86_64_PAGE_SIZE - (vir_addr % X86_64_PAGE_SIZE);
	vir_addr += len;
	next_phys = phys + len;

	while (len < bytes) {
		if (vm_lookup(proc, vir_addr, &phys, NULL) != OK)
			break;
		if (next_phys != phys)
			break;
		len += X86_64_PAGE_SIZE;
		vir_addr += X86_64_PAGE_SIZE;
		next_phys += X86_64_PAGE_SIZE;
	}

	return MIN(bytes, len);
}

int vm_check_range(struct proc *caller, struct proc *target,
	vir_bytes vir_addr, size_t bytes, int writeflag)
{
	int r;

	if ((caller->p_misc_flags & MF_KCALL_RESUME) &&
			(r = caller->p_vmrequest.vmresult) != OK)
		return r;

	vm_suspend(caller, target, vir_addr, bytes, VMSTYPE_KERNELCALL,
		writeflag);

	return VMSUSPEND;
}

int vm_memset(struct proc* caller, endpoint_t who, phys_bytes ph, int c,
	phys_bytes count)
{
	u64_t pattern;
	struct proc *whoptr = NULL;
	phys_bytes cur_ph = ph;
	phys_bytes left = count;
	phys_bytes ptr, chunk, pfa = 0;
	int new_cr3, r = OK;

	if ((r = check_resumed_caller(caller)) != OK)
		return r;

	if (who != NONE && !(whoptr = endpoint_lookup(who)))
		return ESRCH;

	c &= 0xFF;
	/* Build 64-bit pattern: 8 copies of c.
	 * Use u32_t intermediate to avoid sign-extension when c >= 0x80. */
	{
		u32_t pat32 = (unsigned)(c | (c << 8) | (c << 16) | (c << 24));
		pattern = (u64_t)pat32 | ((u64_t)pat32 << 32);
	}

	assert(get_cpulocal_var(ptproc)->p_seg.p_cr3_v);
	assert(!catch_pagefaults);
	catch_pagefaults = 1;

	while (left > 0) {
		new_cr3 = 0;
		chunk = left;
		ptr = createpde(whoptr, cur_ph, &chunk, 0, &new_cr3);

		if (new_cr3)
			reload_cr3();

		if ((pfa = phys_memset(ptr, pattern, chunk))) {
			if (whoptr) {
				vm_suspend(caller, whoptr, ph, count,
					   VMSTYPE_KERNELCALL, 1);
				assert(catch_pagefaults);
				catch_pagefaults = 0;
				return VMSUSPEND;
			}
			panic("vm_memset: pf %lx addr=%lx len=%lu\n",
						pfa , ptr, chunk);
		}

		cur_ph += chunk;
		left -= chunk;
	}

	assert(get_cpulocal_var(ptproc)->p_seg.p_cr3_v);
	assert(catch_pagefaults);
	catch_pagefaults = 0;

	return OK;
}

int virtual_copy_f(
  struct proc * caller,
  struct vir_addr *src_addr,
  struct vir_addr *dst_addr,
  vir_bytes bytes,
  int vmcheck
)
{
  struct vir_addr *vir_addr[2];
  int i, r;
  struct proc *procs[2];

  assert((vmcheck && caller) || (!vmcheck && !caller));

  if (bytes <= 0) return(EDOM);

  vir_addr[_SRC_] = src_addr;
  vir_addr[_DST_] = dst_addr;

  for (i=_SRC_; i<=_DST_; i++) {
  	endpoint_t proc_e = vir_addr[i]->proc_nr_e;
	int proc_nr;
	struct proc *p;

	if(proc_e == NONE) {
		p = NULL;
	} else {
		if(!isokendpt(proc_e, &proc_nr)) {
			printf("virtual_copy: no reasonable endpoint\n");
			return ESRCH;
		}
		p = proc_addr(proc_nr);
	}
	procs[i] = p;
  }

  if ((r = check_resumed_caller(caller)) != OK)
	return r;

  if((r=lin_lin_copy(procs[_SRC_], vir_addr[_SRC_]->offset,
  	procs[_DST_], vir_addr[_DST_]->offset, bytes)) != OK) {
	int writeflag;
  	struct proc *target = NULL;
  	phys_bytes lin;
  	if(r != EFAULT_SRC && r != EFAULT_DST)
  		panic("lin_lin_copy failed: %d", r);
  	if(!vmcheck || !caller) {
    		return r;
  	}

  	if(r == EFAULT_SRC) {
  		lin = vir_addr[_SRC_]->offset;
  		target = procs[_SRC_];
		writeflag = 0;
  	} else if(r == EFAULT_DST) {
  		lin = vir_addr[_DST_]->offset;
  		target = procs[_DST_];
		writeflag = 1;
  	} else {
  		panic("r strange: %d", r);
  	}

	assert(caller);
	assert(target);

	vm_suspend(caller, target, lin, bytes, VMSTYPE_KERNELCALL, writeflag);
	return VMSUSPEND;
  }

  return OK;
}

int data_copy(const endpoint_t from_proc, const vir_bytes from_addr,
	const endpoint_t to_proc, const vir_bytes to_addr,
	size_t bytes)
{
  struct vir_addr src, dst;

  src.offset = from_addr;
  dst.offset = to_addr;
  src.proc_nr_e = from_proc;
  dst.proc_nr_e = to_proc;
  assert(src.proc_nr_e != NONE);
  assert(dst.proc_nr_e != NONE);

  return virtual_copy(&src, &dst, bytes);
}

int data_copy_vmcheck(struct proc * caller,
	const endpoint_t from_proc, const vir_bytes from_addr,
	const endpoint_t to_proc, const vir_bytes to_addr,
	size_t bytes)
{
  struct vir_addr src, dst;

  src.offset = from_addr;
  dst.offset = to_addr;
  src.proc_nr_e = from_proc;
  dst.proc_nr_e = to_proc;
  assert(src.proc_nr_e != NONE);
  assert(dst.proc_nr_e != NONE);

  return virtual_copy_vmcheck(caller, &src, &dst, bytes);
}

void memory_init(void)
{
	assert(nfreepdes == 0);

	freepdes[nfreepdes++] = kinfo.freepde_start++;
	freepdes[nfreepdes++] = kinfo.freepde_start++;

	assert(kinfo.freepde_start < X86_64_VM_DIR_ENTRIES);
	assert(nfreepdes == 2);
	assert(nfreepdes <= MAXFREEPDES);
}

void arch_proc_init(struct proc *pr, const u32_t ip, const u32_t sp,
	const u32_t ps_str, char *name)
{
	arch_proc_reset(pr);
	strlcpy(pr->p_name, name, sizeof(pr->p_name));

	pr->p_reg.pc = ip;
	pr->p_reg.sp = sp;
	pr->p_reg.bx = ps_str;
}

static int oxpcie_mapping_index = -1,
	lapic_mapping_index = -1,
	ioapic_first_index = -1,
	ioapic_last_index = -1,
	video_mem_mapping_index = -1,
	usermapped_glo_index = -1,
	usermapped_index = -1, first_um_idx = -1;

extern char *video_mem;

extern char usermapped_start, usermapped_end, usermapped_nonglo_start;

int arch_phys_map(const int index,
			phys_bytes *addr,
			phys_bytes *len,
			int *flags)
{
	static int first = 1;
	int freeidx = 0;
	static char *ser_var = NULL;
	u32_t glo_len = (u32_t) &usermapped_nonglo_start -
			(u32_t) &usermapped_start;

	if(first) {
		memset(&minix_kerninfo, 0, sizeof(minix_kerninfo));
		video_mem_mapping_index = freeidx++;
		if(glo_len > 0) {
			usermapped_glo_index = freeidx++;
		}

		usermapped_index = freeidx++;
		first_um_idx = usermapped_index;
		if(usermapped_glo_index != -1)
			first_um_idx = usermapped_glo_index;

#ifdef USE_APIC
		if(lapic_addr)
			lapic_mapping_index = freeidx++;
		if (ioapic_enabled) {
			ioapic_first_index = freeidx;
			assert(nioapics > 0);
			freeidx += nioapics;
			ioapic_last_index = freeidx-1;
		}
#endif

#ifdef CONFIG_OXPCIE
		if((ser_var = env_get("oxpcie"))) {
			if(ser_var[0] != '0' || ser_var[1] != 'x') {
				printf("oxpcie address in hex please\n");
			} else {
				printf("oxpcie address is %s\n", ser_var);
				oxpcie_mapping_index = freeidx++;
			}
		}
#endif

		first = 0;
	}

	if(index == usermapped_glo_index) {
		*addr = vir2phys(&usermapped_start);
		*len = glo_len;
		*flags = VMMF_USER | VMMF_GLO;
		return OK;
	}
	else if(index == usermapped_index) {
		*addr = vir2phys(&usermapped_nonglo_start);
		*len = (u32_t) &usermapped_end -
			(u32_t) &usermapped_nonglo_start;
		*flags = VMMF_USER;
		return OK;
	}
	else if (index == video_mem_mapping_index) {
		*addr = MULTIBOOT_VIDEO_BUFFER;
		*len = X86_64_PAGE_SIZE;
		*flags = VMMF_WRITE;
		return OK;
	}
#ifdef USE_APIC
	else if (index == lapic_mapping_index) {
		if (!lapic_addr)
			return EINVAL;
		*addr = lapic_addr;
		*len = 4 << 10;
		*flags = VMMF_UNCACHED | VMMF_WRITE;
		return OK;
	}
	else if (ioapic_enabled && index >= ioapic_first_index && index <= ioapic_last_index) {
		int ioapic_idx = index - ioapic_first_index;
		*addr = io_apic[ioapic_idx].paddr;
		assert(*addr);
		*len = 4 << 10;
		*flags = VMMF_UNCACHED | VMMF_WRITE;
		printf("ioapic map: addr 0x%lx\n", *addr);
		return OK;
	}
#endif

#if CONFIG_OXPCIE
	if(index == oxpcie_mapping_index) {
		*addr = strtoul(ser_var+2, NULL, 16);
		*len = 0x4000;
		*flags = VMMF_UNCACHED | VMMF_WRITE;
		return OK;
	}
#endif

	return EINVAL;
}

int arch_phys_map_reply(const int index, const vir_bytes addr)
{
#ifdef USE_APIC
	if (index == lapic_mapping_index && lapic_addr) {
		lapic_addr_vaddr = addr;
		return OK;
	}
	else if (ioapic_enabled && index >= ioapic_first_index &&
		index <= ioapic_last_index) {
		int i = index - ioapic_first_index;
		io_apic[i].vaddr = addr;
		return OK;
	}
#endif

#if CONFIG_OXPCIE
	if (index == oxpcie_mapping_index) {
		oxpcie_set_vaddr((unsigned char *) addr);
		return OK;
	}
#endif
	if(index == first_um_idx) {
		extern struct minix_ipcvecs minix_ipcvecs_sysenter,
			minix_ipcvecs_syscall,
			minix_ipcvecs_softint;
		extern u64_t usermapped_offset;
		assert(addr > (u64_t) &usermapped_start);
		usermapped_offset = addr - (u64_t) &usermapped_start;
#define FIXEDPTR(ptr) (void *) ((u64_t)ptr + usermapped_offset)
#define FIXPTR(ptr) ptr = FIXEDPTR(ptr)
#define ASSIGN(minixstruct) minix_kerninfo.minixstruct = FIXEDPTR(&minixstruct)
		ASSIGN(kinfo);
		ASSIGN(machine);
		ASSIGN(kmessages);
		ASSIGN(loadinfo);
		ASSIGN(kuserinfo);
		ASSIGN(arm_frclock);
		ASSIGN(kclockinfo);

		if(minix_feature_flags & MKF_I386_INTEL_SYSENTER) {
			DEBUGBASIC(("kernel: selecting intel sysenter ipc style\n"));
			minix_kerninfo.minix_ipcvecs = &minix_ipcvecs_sysenter;
		} else if(minix_feature_flags & MKF_I386_AMD_SYSCALL) {
			DEBUGBASIC(("kernel: selecting amd syscall ipc style\n"));
			minix_kerninfo.minix_ipcvecs = &minix_ipcvecs_syscall;
		} else {
			DEBUGBASIC(("kernel: selecting fallback (int) ipc style\n"));
			minix_kerninfo.minix_ipcvecs = &minix_ipcvecs_softint;
		}

		FIXPTR(minix_kerninfo.minix_ipcvecs->send);
		FIXPTR(minix_kerninfo.minix_ipcvecs->receive);
		FIXPTR(minix_kerninfo.minix_ipcvecs->sendrec);
		FIXPTR(minix_kerninfo.minix_ipcvecs->senda);
		FIXPTR(minix_kerninfo.minix_ipcvecs->sendnb);
		FIXPTR(minix_kerninfo.minix_ipcvecs->notify);
		FIXPTR(minix_kerninfo.minix_ipcvecs->do_kernel_call);
		FIXPTR(minix_kerninfo.minix_ipcvecs);

		minix_kerninfo.kerninfo_magic = KERNINFO_MAGIC;
		minix_kerninfo.minix_feature_flags = minix_feature_flags;
		minix_kerninfo_user = (vir_bytes) FIXEDPTR(&minix_kerninfo);

		if(env_get("libc_ipc")) {
			printf("kernel: forcing in-libc fallback ipc style\n");
			minix_kerninfo.minix_ipcvecs = NULL;
		} else {
			minix_kerninfo.ki_flags |= MINIX_KIF_IPCVECS;
		}

		minix_kerninfo.ki_flags |= MINIX_KIF_USERINFO;

		return OK;
	}

	if(index == usermapped_index) return OK;

	if (index == video_mem_mapping_index) {
		video_mem_vaddr = addr;
		return OK;
	}

	return EINVAL;
}

int arch_enable_paging(struct proc * caller)
{
	assert(caller->p_seg.p_cr3);

	switch_address_space(caller);

	video_mem = (char *) video_mem_vaddr;

#ifdef USE_APIC
	if (lapic_addr) {
		lapic_addr = lapic_addr_vaddr;
		lapic_eoi_addr = LAPIC_EOI;
	}
	if (ioapic_enabled) {
		int i;
		for (i = 0; i < nioapics; i++) {
			io_apic[i].addr = io_apic[i].vaddr;
		}
	}
#if CONFIG_SMP
	barrier();
	wait_for_APs_to_finish_booting();
#endif
#endif

#ifdef USE_WATCHDOG
	if (watchdog_enabled)
		i386_watchdog_start();
#endif

	return OK;
}

void release_address_space(struct proc *pr)
{
	pr->p_seg.p_cr3_v = NULL;
}

int platform_tbl_checksum_ok(void *ptr, unsigned int length)
{
	u8_t total = 0;
	unsigned int i;
	for (i = 0; i < length; i++)
		total += ((unsigned char *)ptr)[i];
	return !total;
}

int platform_tbl_ptr(phys_bytes start,
					phys_bytes end,
					unsigned increment,
					void * buff,
					unsigned size,
					phys_bytes * phys_addr,
					int ((* cmp_f)(void *)))
{
	phys_bytes addr;

	for (addr = start; addr < end; addr += increment) {
		phys_copy (addr, (phys_bytes) buff, size);
		if (cmp_f(buff)) {
			if (phys_addr)
				*phys_addr = addr;
			return 1;
		}
	}
	return 0;
}
