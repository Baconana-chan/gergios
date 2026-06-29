/*	x86_64 rwlock.h — rwlock definitions	*/

#ifndef _X86_64_RWLOCK_H_
#define	_X86_64_RWLOCK_H_

struct krwlock {
	volatile uintptr_t	rw_owner;
};

#ifdef __RWLOCK_PRIVATE

#define	__HAVE_SIMPLE_RW_LOCKS		1

#ifdef MULTIPROCESSOR
#define	RW_RECEIVE(rw)			__asm __volatile("" ::: "memory")
#define	RW_GIVE(rw)			__asm __volatile("" ::: "memory")
#else
#define	RW_RECEIVE(rw)			/* nothing */
#define	RW_GIVE(rw)			/* nothing */
#endif

#define	RW_CAS(p, o, n)			\
    (atomic_cas_ulong((volatile unsigned long *)(p), (o), (n)) == (o))

#endif	/* __RWLOCK_PRIVATE */

#endif /* _X86_64_RWLOCK_H_ */
