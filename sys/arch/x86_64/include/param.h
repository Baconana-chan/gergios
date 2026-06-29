/*	x86_64 param.h — machine parameters	*/

#ifndef _X86_64_PARAM_H_
#define _X86_64_PARAM_H_

/* x86_64 uses 4KB pages */
#define	PGSHIFT		12		/* LOG2(NBPG) */
#define	NBPG		(1 << PGSHIFT)	/* bytes/page */
#define	PGOFSET		(NBPG - 1)	/* byte offset into page */

#endif /* _X86_64_PARAM_H_ */
