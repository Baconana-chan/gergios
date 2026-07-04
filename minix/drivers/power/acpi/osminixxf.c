/******************************************************************************
 *
 * Module Name: osminixxf - MINIX3 OSL interfaces
 *
 *****************************************************************************/

/*
 * Copyright (C) 2000 - 2014, Intel Corp.
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions, and the following disclaimer,
 *    without modification.
 * 2. Redistributions in binary form must reproduce at minimum a disclaimer
 *    substantially similar to the "NO WARRANTY" disclaimer below
 *    ("Disclaimer") and any redistribution must be conditioned upon
 *    including a substantially similar Disclaimer requirement for further
 *    binary redistribution.
 * 3. Neither the names of the above-listed copyright holders nor the names
 *    of any contributors may be used to endorse or promote products derived
 *    from this software without specific prior written permission.
 *
 * Alternatively, this software may be distributed under the terms of the
 * GNU General Public License ("GPL") version 2 as published by the Free
 * Software Foundation.
 *
 * NO WARRANTY
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTIBILITY AND FITNESS FOR
 * A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
 * HOLDERS OR CONTRIBUTORS BE LIABLE FOR SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGES.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/time.h>

#include "acpi.h"
#include "accommon.h"
#include "amlcode.h"
#include "acparser.h"
#include "acdebug.h"

#include <minix/driver.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <machine/pci_intel.h>

/* Maximum number of deferred execution callbacks */
#define ACPI_OS_EXECUTE_QUEUE_MAX 32

/* ACPI semaphore struct — counting semaphore */
struct acpi_semaphore {
	int		initialized;
	u32_t		counter;
	u32_t		max_units;
};

/* Deferred execution queue item */
struct acpi_exec_item {
	ACPI_OSD_EXEC_CALLBACK	function;
	void			*context;
};

/* ACPI SCI interrupt state */
static int acpi_sci_irq = -1;
static int acpi_sci_hook_id = -1;
static ACPI_OSD_HANDLER acpi_sci_handler = NULL;
static void *acpi_sci_context = NULL;

/* Deferred execution queue */
static struct acpi_exec_item acpi_exec_queue[ACPI_OS_EXECUTE_QUEUE_MAX];
static int acpi_exec_head = 0;
static int acpi_exec_tail = 0;

/* Simple spinlock for semaphore ops (userspace — no real kernel spinlock needed) */
static int acpi_lock = 0;

extern struct machine machine;


static void acpi_spin_lock(void)
{
	while (__sync_lock_test_and_set(&acpi_lock, 1))
		usleep(1);
}

static void acpi_spin_unlock(void)
{
	__sync_lock_release(&acpi_lock);
}

static u32_t pci_inb(u16_t port) {
	u32_t value;
	int s;
	if ((s=sys_inb(port, &value)) !=OK)
		printf("ACPI: warning, sys_inb failed: %d\n", s);
	return value;
}

static u32_t pci_inw(u16_t port) {
	u32_t value;
	int s;
	if ((s=sys_inw(port, &value)) !=OK)
		printf("ACPI: warning, sys_inw failed: %d\n", s);
	return value;
}

static u32_t pci_inl(u16_t port) {
	u32_t value;
	int s;
	if ((s=sys_inl(port, &value)) !=OK)
		printf("ACPI: warning, sys_inl failed: %d\n", s);
	return value;
}

static void pci_outb(u16_t port, u8_t value) {
	int s;
	if ((s=sys_outb(port, value)) !=OK)
		printf("ACPI: warning, sys_outb failed: %d\n", s);
}

static void pci_outw(u16_t port, u16_t value) {
	int s;
	if ((s=sys_outw(port, value)) !=OK)
		printf("ACPI: warning, sys_outw failed: %d\n", s);
}

static void pci_outl(u16_t port, u32_t value) {
	int s;
	if ((s=sys_outl(port, value)) !=OK)
		printf("ACPI: warning, sys_outl failed: %d\n", s);
}

/******************************************************************************
 *
 * FUNCTION:    AcpiOsInitialize, AcpiOsTerminate
 *
 * PARAMETERS:  None
 *
 * RETURN:      Status
 *
 * DESCRIPTION: Init and terminate.  Nothing to do.
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsInitialize (void)
{
	return AE_OK;
}


ACPI_STATUS
AcpiOsTerminate (void)
{
	return AE_OK;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsPredefinedOverride
 *
 * PARAMETERS:  InitVal     - Initial value of the predefined object
 *              NewVal      - The new value for the object
 *
 * RETURN:      Status, pointer to value.  Null pointer returned if not
 *              overriding.
 *
 * DESCRIPTION: Allow the OS to override predefined names
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsPredefinedOverride (
    const ACPI_PREDEFINED_NAMES *InitVal,
    ACPI_STRING                 *NewVal)
{
	*NewVal = NULL;
	return (AE_OK);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsTableOverride
 *
 * PARAMETERS:  ExistingTable   - Header of current table (probably firmware)
 *              NewTable        - Where an entire new table is returned.
 *
 * RETURN:      Status, pointer to new table.  Null pointer returned if no
 *              table is available to override
 *
 * DESCRIPTION: Return a different version of a table if one is available
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsTableOverride (
    ACPI_TABLE_HEADER       *ExistingTable,
    ACPI_TABLE_HEADER       **NewTable)
{
	*NewTable = NULL;
	return (AE_OK);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsReadable
 *
 * PARAMETERS:  Pointer             - Area to be verified
 *              Length              - Size of area
 *
 * RETURN:      TRUE if readable for entire length
 *
 * DESCRIPTION: Verify that a pointer is valid for reading
 *
 *****************************************************************************/

BOOLEAN
AcpiOsReadable (
    void                    *Pointer,
    ACPI_SIZE               Length)
{
	panic("NOTIMPLEMENTED %s\n", __func__);

	return (TRUE);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsWritable
 *
 * PARAMETERS:  Pointer             - Area to be verified
 *              Length              - Size of area
 *
 * RETURN:      TRUE if writable for entire length
 *
 * DESCRIPTION: Verify that a pointer is valid for writing
 *
 *****************************************************************************/

BOOLEAN
AcpiOsWritable (
    void                    *Pointer,
    ACPI_SIZE               Length)
{
	panic("NOTIMPLEMENTED %s\n", __func__);

	return (TRUE);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsPhysicalTableOverride
 *
 * PARAMETERS:  ExistingTable       - Header of current table (probably firmware)
 *              NewAddress          - Where new table address is returned
 *                                    (Physical address)
 *              NewTableLength      - Where new table length is returned
 *
 * RETURN:      Status, address/length of new table. Null pointer returned
 *              if no table is available to override.
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsPhysicalTableOverride (
    ACPI_TABLE_HEADER       *ExistingTable,
    ACPI_PHYSICAL_ADDRESS   *NewAddress,
    UINT32                  *NewTableLength)
{
	*NewAddress = 0;
	return (AE_OK);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsRedirectOutput
 *
 * PARAMETERS:  Destination         - An open file handle/pointer
 *
 * RETURN:      None
 *
 * DESCRIPTION: Causes redirect of AcpiOsPrintf and AcpiOsVprintf
 *
 *****************************************************************************/

void
AcpiOsRedirectOutput (
    void                    *Destination)
{
	panic("NOTIMPLEMENTED %s\n", __func__);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsPrintf
 *
 * PARAMETERS:  fmt, ...            Standard printf format
 *
 * RETURN:      None
 *
 * DESCRIPTION: Formatted output
 *
 *****************************************************************************/

void ACPI_INTERNAL_VAR_XFACE
AcpiOsPrintf (
    const char              *Fmt,
    ...)
{
	va_list                 Args;


	va_start (Args, Fmt);

#ifdef ACPI_BF_DEBUG
	AcpiOsVprintf (Fmt, Args);
#endif

	va_end (Args);
	return;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsVprintf
 *
 * PARAMETERS:  fmt                 Standard printf format
 *              args                Argument list
 *
 * RETURN:      None
 *
 * DESCRIPTION: Formatted output with argument list pointer
 *
 *****************************************************************************/

void
AcpiOsVprintf (
    const char              *Fmt,
    va_list                 Args)
{

	printf("ACPI: ");
	vprintf (Fmt, Args);
	printf("\n");
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsGetLine
 *
 * PARAMETERS:  Buffer              - Where to return the command line
 *              BufferLength        - Maximum length of Buffer
 *              BytesRead           - Where the actual byte count is returned
 *
 * RETURN:      Status and actual bytes read
 *
 * DESCRIPTION: Get the next input line from the terminal. NOTE: For the
 *              AcpiExec utility, we use the acgetline module instead to
 *              provide line-editing and history support.
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsGetLine (
    char                    *Buffer,
    UINT32                  BufferLength,
    UINT32                  *BytesRead)
{
	panic("NOTIMPLEMENTED %s\n", __func__);
	return 0;
}

/******************************************************************************
 *
 * FUNCTION:    AcpiOsMapMemory
 *
 * PARAMETERS:  where               Physical address of memory to be mapped
 *              length              How much memory to map
 *
 * RETURN:      Pointer to mapped memory.  Null on error.
 *
 * DESCRIPTION: Map physical memory into caller's address space
 *
 *****************************************************************************/

void *
AcpiOsMapMemory (
    ACPI_PHYSICAL_ADDRESS   where,  /* not page aligned */
    ACPI_SIZE               length) /* in bytes, not page-aligned */
{
	return vm_map_phys(SELF, (void *) where, length);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsUnmapMemory
 *
 * PARAMETERS:  where               Logical address of memory to be unmapped
 *              length              How much memory to unmap
 *
 * RETURN:      None.
 *
 * DESCRIPTION: Delete a previously created mapping.  Where and Length must
 *              correspond to a previous mapping exactly.
 *
 *****************************************************************************/

void
AcpiOsUnmapMemory (
    void                    *where,
    ACPI_SIZE               length)
{
	vm_unmap_phys(SELF, where, length);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsAllocate
 *
 * PARAMETERS:  Size                Amount to allocate, in bytes
 *
 * RETURN:      Pointer to the new allocation.  Null on error.
 *
 * DESCRIPTION: Allocate memory.  Algorithm is dependent on the OS.
 *
 *****************************************************************************/

void *
AcpiOsAllocate (
    ACPI_SIZE               size)
{
	void                    *Mem;


	Mem = (void *) malloc ((size_t) size);
	if (Mem == NULL)
		printf("AcpiOsAllocate out of memory\n");

	return Mem;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsFree
 *
 * PARAMETERS:  mem                 Pointer to previously allocated memory
 *
 * RETURN:      None.
 *
 * DESCRIPTION: Free memory allocated via AcpiOsAllocate
 *
 *****************************************************************************/

void
AcpiOsFree (
    void                    *mem)
{
	free(mem);
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsCreateSemaphore
 *
 * PARAMETERS:  InitialUnits        - Units to be assigned to the new semaphore
 *              OutHandle           - Where a handle will be returned
 *
 * RETURN:      Status
 *
 * DESCRIPTION: Create an OS semaphore
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsCreateSemaphore (
    UINT32              MaxUnits,
    UINT32              InitialUnits,
    ACPI_HANDLE         *OutHandle)
{
	struct acpi_semaphore *sem;

	if (!OutHandle)
		return AE_BAD_PARAMETER;

	sem = AcpiOsAllocate(sizeof(struct acpi_semaphore));
	if (!sem)
		return AE_NO_MEMORY;

	sem->initialized = 1;
	sem->counter = InitialUnits;
	sem->max_units = MaxUnits;
	sem->blocked_endpoint = NONE;

	*OutHandle = (ACPI_HANDLE)sem;
	return AE_OK;
}

ACPI_STATUS
AcpiOsDeleteSemaphore (
    ACPI_HANDLE         Handle)
{
	struct acpi_semaphore *sem = (struct acpi_semaphore *)Handle;

	if (!sem || !sem->initialized)
		return AE_BAD_PARAMETER;

	sem->initialized = 0;
	AcpiOsFree(sem);
	return AE_OK;
}

ACPI_STATUS
AcpiOsWaitSemaphore (
    ACPI_HANDLE         Handle,
    UINT32              Units,
    UINT16              Timeout)
{
	struct acpi_semaphore *sem = (struct acpi_semaphore *)Handle;
	int timed_out = 0;
	int total_us = 0;
	int timeout_us;

	if (!sem || !sem->initialized || Units == 0)
		return AE_BAD_PARAMETER;

	timeout_us = (Timeout == ACPI_NO_UNIT_LIMIT) ?
	    -1 : (int)Timeout * 1000;

	for (;;) {
		acpi_spin_lock();
		if (sem->counter >= Units) {
			sem->counter -= Units;
			acpi_spin_unlock();
			return AE_OK;
		}
		acpi_spin_unlock();

		if (timed_out)
			return AE_TIME;

		usleep(1000);
		total_us += 1000;
		if (timeout_us >= 0 && total_us >= timeout_us)
			timed_out = 1;
	}
}

ACPI_STATUS
AcpiOsSignalSemaphore (
    ACPI_HANDLE         Handle,
    UINT32              Units)
{
	struct acpi_semaphore *sem = (struct acpi_semaphore *)Handle;

	if (!sem || !sem->initialized)
		return AE_BAD_PARAMETER;

	acpi_spin_lock();
	if (sem->counter + Units <= sem->max_units)
		sem->counter += Units;
	else
		sem->counter = sem->max_units;
	acpi_spin_unlock();

	return AE_OK;
}


ACPI_STATUS
AcpiOsCreateLock (
    ACPI_SPINLOCK           *OutHandle)
{
	*OutHandle = NULL;
	return AE_OK;
}

void
AcpiOsDeleteLock (
    ACPI_SPINLOCK           Handle)
{
}


ACPI_CPU_FLAGS
AcpiOsAcquireLock (
    ACPI_HANDLE             Handle)
{
	return (0);
}


void
AcpiOsReleaseLock (
    ACPI_SPINLOCK           Handle,
    ACPI_CPU_FLAGS          Flags)
{
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsInstallInterruptHandler
 *
 * PARAMETERS:  InterruptNumber     Level handler should respond to.
 *              Isr                 Address of the ACPI interrupt handler
 *              ExceptPtr           Where status is returned
 *
 * RETURN:      Handle to the newly installed handler.
 *
 * DESCRIPTION: Install an interrupt handler.  Used to install the ACPI
 *              OS-independent handler.
 *
 *****************************************************************************/

/*
 * Tell the main loop to dispatch the SCI when a HARDWARE notification arrives.
 * The ACPICA handler is called from the main loop context, not from interrupt
 * context (MINIX userspace drivers cannot call arbitrary code in ring 0).
 */
UINT32
AcpiOsInstallInterruptHandler (
    UINT32                  InterruptNumber,
    ACPI_OSD_HANDLER        ServiceRoutine,
    void                    *Context)
{
	int r;

	if (acpi_sci_irq >= 0) {
		printf("ACPI: SCI interrupt already installed on IRQ %d\n",
		    acpi_sci_irq);
		return AE_ALREADY_EXISTS;
	}

	acpi_sci_handler = ServiceRoutine;
	acpi_sci_context = Context;
	acpi_sci_irq = InterruptNumber;

	/*
	 * Register for the SCI interrupt. The kernel will send a HARDWARE
	 * notification via the standard MINIX IRQ mechanism. The main loop
	 * in acpi.c dispatches notifications by calling acpi_dispatch_sci().
	 */
	r = sys_irqsetpolicy(InterruptNumber, 0, &acpi_sci_hook_id);
	if (r != OK) {
		printf("ACPI: sys_irqsetpolicy(IRQ %d) failed: %d\n",
		    InterruptNumber, r);
		acpi_sci_irq = -1;
		return AE_ERROR;
	}

	r = sys_irqenable(&acpi_sci_hook_id);
	if (r != OK) {
		printf("ACPI: sys_irqenable(IRQ %d) failed: %d\n",
		    InterruptNumber, r);
		sys_irqrmpolicy(&acpi_sci_hook_id);
		acpi_sci_irq = -1;
		return AE_ERROR;
	}

	/* SCI is critical system event — SCHED_FIFO 98 (just below timer 99) */
	sys_irqthread_priority(InterruptNumber, 98);

	printf("ACPI: SCI interrupt handler installed on IRQ %d\n",
	    InterruptNumber);
	return AE_OK;
}

ACPI_STATUS
AcpiOsRemoveInterruptHandler (
    UINT32                  InterruptNumber,
    ACPI_OSD_HANDLER        ServiceRoutine)
{
	int r;

	if (acpi_sci_irq != (int)InterruptNumber) {
		printf("ACPI: SCI IRQ %d mismatch with installed %d\n",
		    InterruptNumber, acpi_sci_irq);
		return AE_NOT_EXIST;
	}

	r = sys_irqrmpolicy(&acpi_sci_hook_id);
	if (r != OK) {
		printf("ACPI: sys_irqrmpolicy failed: %d\n", r);
		return AE_ERROR;
	}

	acpi_sci_handler = NULL;
	acpi_sci_context = NULL;
	acpi_sci_irq = -1;
	acpi_sci_hook_id = -1;

	return AE_OK;
}

/* Called from acpi.c main loop when HARDWARE notification arrives */
void acpi_dispatch_sci(void)
{
	if (acpi_sci_handler) {
		acpi_sci_handler(acpi_sci_context);
	} else {
		printf("ACPI: SCI received but no handler registered\n");
	}
}

/* Called from acpi.c main loop to process deferred execution queue */
int acpi_os_process_exec_queue(void)
{
	int processed = 0;

	while (acpi_exec_head != acpi_exec_tail && processed < 16) {
		acpi_spin_lock();
		int idx = acpi_exec_head;
		acpi_exec_head = (acpi_exec_head + 1) % ACPI_OS_EXECUTE_QUEUE_MAX;
		acpi_spin_unlock();

		struct acpi_exec_item *item = &acpi_exec_queue[idx];
		if (item->function) {
			item->function(item->context);
			memset(item, 0, sizeof(*item));
		}
		processed++;
	}

	return processed;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsExecute
 *
 * PARAMETERS:  Type            - Type of execution
 *              Function        - Address of the function to execute
 *              Context         - Passed as a parameter to the function
 *
 * RETURN:      Status.
 *
 * DESCRIPTION: Execute a new thread
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsExecute (
    ACPI_EXECUTE_TYPE       Type,
    ACPI_OSD_EXEC_CALLBACK  Function,
    void                    *Context)
{
	int next;

	if (!Function)
		return AE_BAD_PARAMETER;

	acpi_spin_lock();
	next = (acpi_exec_tail + 1) % ACPI_OS_EXECUTE_QUEUE_MAX;

	if (next == acpi_exec_head) {
		/* Queue full — fall back to direct execution */
		acpi_spin_unlock();
		printf("ACPI: execute queue full, running directly\n");
		Function(Context);
		return AE_OK;
	}

	acpi_exec_queue[acpi_exec_tail].function = Function;
	acpi_exec_queue[acpi_exec_tail].context = Context;
	acpi_exec_tail = next;
	acpi_spin_unlock();

	return AE_OK;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsBreakpoint
 *
 * PARAMETERS:  Msg                 Message to print
 *
 * RETURN:      Status
 *
 * DESCRIPTION: Print a message and break to the debugger.
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsBreakpoint (
    char                    *Msg)
{
	panic("NOTIMPLEMENTED %s\n", __func__);
	return AE_OK;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsStall
 *
 * PARAMETERS:  microseconds        To sleep
 *
 * RETURN:      Blocks until sleep is completed.
 *
 * DESCRIPTION: Sleep at microsecond granularity
 *
 *****************************************************************************/

void
AcpiOsStall (
    UINT32                  microseconds)
{
	if (microseconds > 0)
		usleep (microseconds);

	return;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsSleep
 *
 * PARAMETERS:  milliseconds        To sleep
 *
 * RETURN:      Blocks until sleep is completed.
 *
 * DESCRIPTION: Sleep at millisecond granularity
 *
 *****************************************************************************/

void
AcpiOsSleep (
    ACPI_INTEGER            milliseconds)
{
	if ((milliseconds / 1000) > 0)
		sleep (milliseconds / 1000);

	if ((milliseconds % 1000) > 0)
		usleep ((milliseconds % 1000) * 1000);

	return;
}

/******************************************************************************
 *
 * FUNCTION:    AcpiOsGetTimer
 *
 * PARAMETERS:  None
 *
 * RETURN:      Current time in 100 nanosecond units
 *
 * DESCRIPTION: Get the current system time
 *
 *****************************************************************************/

UINT64
AcpiOsGetTimer (void)
{
	struct timeval	time;

	gettimeofday (&time, NULL);
	return (((UINT64) time.tv_sec * 10000000) +
		((UINT64) time.tv_usec * 10));
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsValidateInterface
 *
 * PARAMETERS:  Interface           - Requested interface to be validated
 *
 * RETURN:      AE_OK if interface is supported, AE_SUPPORT otherwise
 *
 * DESCRIPTION: Match an interface string to the interfaces supported by the
 *              host. Strings originate from an AML call to the _OSI method.
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsValidateInterface (
    char                    *Interface)
{
	return (AE_SUPPORT);
}


/* TEMPORARY STUB FUNCTION */
void
AcpiOsDerivePciId(
    ACPI_HANDLE             rhandle,
    ACPI_HANDLE             chandle,
    ACPI_PCI_ID             **PciId)
{
	/* we do nothing here, we keep the PciId unchanged */
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsReadPort
 *
 * PARAMETERS:  Address             Address of I/O port/register to read
 *              Value               Where value is placed
 *              Width               Number of bits
 *
 * RETURN:      Value read from port
 *
 * DESCRIPTION: Read data from an I/O port or register
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsReadPort (
    ACPI_IO_ADDRESS         Address,
    UINT32                  *Value,
    UINT32                  Width)
{
	*Value = 0;
	switch (Width) {
		case 8:
			sys_inb(Address, Value);
			break;
		case 16:
			sys_inw(Address, Value);
			break;
		case 32:
			sys_inl(Address, Value);
			break;
		default:
			panic("unsupported width: %d", Width);
	}
	return AE_OK;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsWritePort
 *
 * PARAMETERS:  Address             Address of I/O port/register to write
 *              Value               Value to write
 *              Width               Number of bits
 *
 * RETURN:      None
 *
 * DESCRIPTION: Write data to an I/O port or register
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsWritePort (
    ACPI_IO_ADDRESS         Address,
    UINT32                  Value,
    UINT32                  Width)
{
	switch (Width) {
		case 8:
			sys_outb(Address, Value);
			break;
		case 16:
			sys_outw(Address, Value);
			break;
		case 32:
			sys_outl(Address, Value);
			break;
		default:
			panic("unsupported width: %d", Width);
	}
	return AE_OK;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsReadMemory
 *
 * PARAMETERS:  Address             Physical Memory Address to read
 *              Value               Where value is placed
 *              Width               Number of bits
 *
 * RETURN:      Value read from physical memory address
 *
 * DESCRIPTION: Read data from a physical memory address
 *
 *****************************************************************************/

/* Cache for the last mapped MMIO region (AcpiOsReadMemory/WriteMemory) */
static struct {
	ACPI_PHYSICAL_ADDRESS	phys_base;
	volatile void		*virt_addr;
	ACPI_SIZE		length;
} acpi_mmio_cache;

/* Map physical memory for MMIO read/write, using a simple cache */
static volatile void *acpi_map_mmio(ACPI_PHYSICAL_ADDRESS addr, ACPI_SIZE width)
{
	ACPI_PHYSICAL_ADDRESS page_base = addr & ~(ACPI_PHYSICAL_ADDRESS)0xFFF;
	ACPI_SIZE map_len = width + (addr - page_base);

	/* Hit in cache? */
	if (acpi_mmio_cache.virt_addr &&
	    addr >= acpi_mmio_cache.phys_base &&
	    addr + width <= acpi_mmio_cache.phys_base + acpi_mmio_cache.length) {
		return acpi_mmio_cache.virt_addr + (addr - acpi_mmio_cache.phys_base);
	}

	/* Unmap old, map new */
	if (acpi_mmio_cache.virt_addr)
		AcpiOsUnmapMemory((void *)acpi_mmio_cache.virt_addr,
		    acpi_mmio_cache.length);

	acpi_mmio_cache.virt_addr = AcpiOsMapMemory(page_base, map_len);
	if (!acpi_mmio_cache.virt_addr)
		return NULL;

	acpi_mmio_cache.phys_base = page_base;
	acpi_mmio_cache.length = map_len;

	return acpi_mmio_cache.virt_addr + (addr - acpi_mmio_cache.phys_base);
}

ACPI_STATUS
AcpiOsReadMemory (
    ACPI_PHYSICAL_ADDRESS   Address,
    UINT64                  *Value,
    UINT32                  Width)
{
	volatile void *ptr;

	if (!Value)
		return AE_BAD_PARAMETER;

	ptr = acpi_map_mmio(Address, Width / 8);
	if (!ptr)
		return AE_NO_MEMORY;

	switch (Width) {
	case 8:
		*Value = *(volatile u8_t *)ptr;
		break;
	case 16:
		*Value = *(volatile u16_t *)ptr;
		break;
	case 32:
		*Value = *(volatile u32_t *)ptr;
		break;
	case 64:
		*Value = *(volatile u64_t *)ptr;
		break;
	default:
		return AE_BAD_PARAMETER;
	}

	return AE_OK;
}

ACPI_STATUS
AcpiOsWriteMemory (
    ACPI_PHYSICAL_ADDRESS   Address,
    UINT64                  Value,
    UINT32                  Width)
{
	volatile void *ptr;

	ptr = acpi_map_mmio(Address, Width / 8);
	if (!ptr)
		return AE_NO_MEMORY;

	switch (Width) {
	case 8:
		*(volatile u8_t *)ptr = (u8_t)Value;
		break;
	case 16:
		*(volatile u16_t *)ptr = (u16_t)Value;
		break;
	case 32:
		*(volatile u32_t *)ptr = (u32_t)Value;
		break;
	case 64:
		*(volatile u64_t *)ptr = (u64_t)Value;
		break;
	default:
		return AE_BAD_PARAMETER;
	}

	return AE_OK;
}


ACPI_THREAD_ID
AcpiOsGetThreadId(void)
{
    return (ACPI_THREAD_ID) 1;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsSignal
 *
 * PARAMETERS:  Function            ACPI CA signal function code
 *              Info                Pointer to function-dependent structure
 *
 * RETURN:      Status
 *
 * DESCRIPTION: Miscellaneous functions
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsSignal (
    UINT32                  Function,
    void                    *Info)
{
	panic("NOTIMPLEMENTED %s\n", __func__);
	return (AE_OK);
}

/******************************************************************************
 *
 * FUNCTION:    AcpiOsGetRootPointer
 *
 * PARAMETERS:  None
 *
 * RETURN:      RSDP physical address
 *
 * DESCRIPTION: Gets the root pointer (RSDP)
 *
 *****************************************************************************/

ACPI_PHYSICAL_ADDRESS AcpiOsGetRootPointer (
    void)
{
	return machine.acpi_rsdp;
}

/******************************************************************************
 *
 * FUNCTION:    AcpiOsReadPciConfiguration
 *
 * PARAMETERS:  PciId               Seg/Bus/Dev
 *              Register            Device Register
 *              Value               Buffer where value is placed
 *              Width               Number of bits
 *
 * RETURN:      Status
 *
 * DESCRIPTION: Read data from PCI configuration space
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsReadPciConfiguration (
    ACPI_PCI_ID             *PciId,
    UINT32                  Register,
    UINT64                  *Value,
    UINT32                  Width)
{
	int err;

	switch (Width) {
		case 8:
			*(u8_t *)Value = PCII_RREG8_(PciId->Bus, PciId->Device,
					PciId->Function, Register);
			break;
		case 16:
			*(u16_t *)Value = PCII_RREG16_(PciId->Bus, PciId->Device,
					PciId->Function, Register);
			break;
		case 32:
			*(u32_t *)Value = PCII_RREG32_(PciId->Bus, PciId->Device,
					PciId->Function, Register);
			break;
		default:
			panic("NOT IMPLEMENTED\n");
	}

	if (OK != (err = sys_outl(PCII_CONFADD, PCII_UNSEL)))
		printf("ACPI: warning, sys_outl failed: %d\n", err);

	return AE_OK;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsWritePciConfiguration
 *
 * PARAMETERS:  PciId               Seg/Bus/Dev
 *              Register            Device Register
 *              Value               Value to be written
 *              Width               Number of bits
 *
 * RETURN:      Status.
 *
 * DESCRIPTION: Write data to PCI configuration space
 *
 *****************************************************************************/

ACPI_STATUS
AcpiOsWritePciConfiguration (
    ACPI_PCI_ID             *PciId,
    UINT32                  Register,
    ACPI_INTEGER            Value,
    UINT32                  Width)
{
	int err;

	switch (Width) {
		case 8:
			PCII_WREG8_(PciId->Bus, PciId->Device,
					PciId->Function, Register, Value);
			break;
		case 16:
			PCII_WREG16_(PciId->Bus, PciId->Device,
					PciId->Function, Register, Value);
			break;
		case 32:
			PCII_WREG32_(PciId->Bus, PciId->Device,
					PciId->Function, Register, Value);
			break;
		default:
			panic("NOT IMPLEMENTED\n");
	}

	if (OK != (err = sys_outl(PCII_CONFADD, PCII_UNSEL)))
		printf("ACPI: warning, sys_outl failed: %d\n", err);

	return AE_OK;
}


/******************************************************************************
 *
 * FUNCTION:    AcpiOsWaitEventsComplete
 *
 * PARAMETERS:  None
 *
 * RETURN:      None
 *
 * DESCRIPTION: Wait for all asynchronous events to complete. This
 *              implementation does nothing.
 *
 *****************************************************************************/

void
AcpiOsWaitEventsComplete (
    void)
{
    return;
}
