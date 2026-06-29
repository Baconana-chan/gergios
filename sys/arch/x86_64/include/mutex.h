/*	x86_64 mutex.h — mutex definitions	*/

#ifndef _X86_64_MUTEX_H_
#define	_X86_64_MUTEX_H_

#ifndef __MUTEX_PRIVATE

struct kmutex {
	uintptr_t	mtx_pad1;
};

#else	/* __MUTEX_PRIVATE */

struct kmutex {
	union {
		/* Adaptive mutex */
		volatile uintptr_t	mtxa_owner;

		/* Spin mutex */
		struct {
			volatile uint8_t	mtxs_dummy;
			ipl_cookie_t		mtxs_ipl;
			__cpu_simple_lock_t	mtxs_lock;
			volatile uint8_t	mtxs_unused;
		} s;
	} u;
};

#define	mtx_owner		u.mtxa_owner
#define	mtx_ipl			u.s.mtxs_ipl
#define	mtx_lock		u.s.mtxs_lock

#define	__HAVE_SIMPLE_MUTEXES		1

#ifdef MULTIPROCESSOR
#define	MUTEX_RECEIVE(mtx)		__asm __volatile("" ::: "memory")
#define	MUTEX_GIVE(mtx)			__asm __volatile("" ::: "memory")
#else
#define	MUTEX_RECEIVE(mtx)		/* nothing */
#define	MUTEX_GIVE(mtx)			/* nothing */
#endif

#define	MUTEX_CAS(p, o, n)		\
    (atomic_cas_ulong((volatile unsigned long *)(p), (o), (n)) == (o))

#ifdef MULTIPROCESSOR
#define	MUTEX_SMT_PAUSE()		__asm __volatile("pause")
#define	MUTEX_SMT_WAKE()		/* nothing */
#endif

#endif	/* __MUTEX_PRIVATE */

#endif /* _X86_64_MUTEX_H_ */
