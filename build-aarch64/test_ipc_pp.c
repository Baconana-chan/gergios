# 1 "test_ipc.c"
# 1 "<built-in>" 1
# 1 "<built-in>" 3
# 412 "<built-in>" 3
# 1 "<command line>" 1
# 1 "<built-in>" 2
# 1 "test_ipc.c" 2
# 1 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h" 1



# 1 "C:/Users/VIC/gergios/minix/include\\minix/ipcconst.h" 1



# 1 "C:/Users/VIC/gergios/sys\\machine/ipcconst.h" 1
# 5 "C:/Users/VIC/gergios/minix/include\\minix/ipcconst.h" 2
# 5 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h" 2
# 1 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 1



# 1 "C:/Users/VIC/gergios/sys\\sys/types.h" 1
# 42 "C:/Users/VIC/gergios/sys\\sys/types.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 43 "C:/Users/VIC/gergios/sys\\sys/types.h" 2


# 1 "C:/Users/VIC/gergios/sys\\machine/types.h" 1




# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\types.h" 1





# 1 "C:/Users/VIC/gergios/sys\\sys/cdefs.h" 1
# 62 "C:/Users/VIC/gergios/sys\\sys/cdefs.h"
# 1 "C:/Users/VIC/gergios/sys\\machine/cdefs.h" 1







# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\cdefs.h" 1
# 9 "C:/Users/VIC/gergios/sys\\machine/cdefs.h" 2
# 63 "C:/Users/VIC/gergios/sys\\sys/cdefs.h" 2

# 1 "C:/Users/VIC/gergios/sys\\sys/cdefs_elf.h" 1
# 65 "C:/Users/VIC/gergios/sys\\sys/cdefs.h" 2
# 596 "C:/Users/VIC/gergios/sys\\sys/cdefs.h"
static __inline long long __zeroll(void) { return 0; }
static __inline unsigned long long __zeroull(void) { return 0; }
# 7 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\types.h" 2
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 8 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\types.h" 2
# 1 "C:/Users/VIC/gergios/sys\\machine/int_types.h" 1




# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\int_types.h" 1






typedef signed char __int8_t;
typedef unsigned char __uint8_t;
typedef short int __int16_t;
typedef unsigned short int __uint16_t;
typedef int __int32_t;
typedef unsigned int __uint32_t;
typedef long int __int64_t;
typedef unsigned long int __uint64_t;






typedef long int __intptr_t;
typedef unsigned long int __uintptr_t;
# 6 "C:/Users/VIC/gergios/sys\\machine/int_types.h" 2
# 9 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\types.h" 2


typedef unsigned long paddr_t;
typedef unsigned long psize_t;
typedef unsigned long vaddr_t;
typedef unsigned long vsize_t;







typedef long register_t;
typedef int register32_t;


typedef unsigned long pmc_evid_t;

typedef unsigned long pmc_ctr_t;
typedef unsigned short tlb_asid_t;

typedef unsigned char __cpu_simple_lock_nv_t;
# 6 "C:/Users/VIC/gergios/sys\\machine/types.h" 2
# 46 "C:/Users/VIC/gergios/sys\\sys/types.h" 2

# 1 "C:/Users/VIC/gergios/sys\\machine/ansi.h" 1




# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\ansi.h" 1
# 6 "C:/Users/VIC/gergios/sys\\machine/ansi.h" 2
# 48 "C:/Users/VIC/gergios/sys\\sys/types.h" 2



# 1 "C:/Users/VIC/gergios/sys\\sys/ansi.h" 1
# 37 "C:/Users/VIC/gergios/sys\\sys/ansi.h"
typedef char * __caddr_t;
typedef __uint32_t __gid_t;
typedef __uint32_t __in_addr_t;
typedef __uint16_t __in_port_t;
typedef __uint32_t __mode_t;
typedef __int64_t __off_t;
typedef __int32_t __pid_t;
typedef __uint8_t __sa_family_t;
typedef unsigned int __socklen_t;
typedef __uint32_t __uid_t;
typedef __uint64_t __fsblkcnt_t;
typedef __uint64_t __fsfilcnt_t;

struct __tag_wctrans_t;
typedef struct __tag_wctrans_t *__wctrans_t;

struct __tag_wctype_t;
typedef struct __tag_wctype_t *__wctype_t;





typedef union {
 __int64_t __mbstateL;
 char __mbstate8[128];
} __mbstate_t;
# 72 "C:/Users/VIC/gergios/sys\\sys/ansi.h"
typedef __builtin_va_list __va_list;
# 52 "C:/Users/VIC/gergios/sys\\sys/types.h" 2


typedef __int8_t int8_t;




typedef __uint8_t uint8_t;




typedef __int16_t int16_t;




typedef __uint16_t uint16_t;




typedef __int32_t int32_t;




typedef __uint32_t uint32_t;




typedef __int64_t int64_t;




typedef __uint64_t uint64_t;



typedef __uint8_t u_int8_t;
typedef __uint16_t u_int16_t;
typedef __uint32_t u_int32_t;
typedef __uint64_t u_int64_t;


typedef __uint8_t u8_t;

typedef __uint16_t u16_t;

typedef __uint32_t u32_t;

typedef __uint64_t u64_t;


typedef __int8_t i8_t;

typedef __int16_t i16_t;

typedef __int32_t i32_t;

typedef __int64_t i64_t;




typedef __uint32_t zone_t;
typedef __uint32_t block_t;
typedef __uint64_t block64_t;
typedef __uint32_t bit_t;
typedef __uint16_t zone1_t;
typedef __uint32_t bitchunk_t;



# 1 "C:/Users/VIC/gergios/sys\\machine/endian.h" 1




# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\endian.h" 1
# 11 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\endian.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/endian.h" 1
# 37 "C:/Users/VIC/gergios/sys\\sys/endian.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 38 "C:/Users/VIC/gergios/sys\\sys/endian.h" 2
# 55 "C:/Users/VIC/gergios/sys\\sys/endian.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/types.h" 1
# 56 "C:/Users/VIC/gergios/sys\\sys/endian.h" 2


typedef __in_addr_t in_addr_t;




typedef __in_port_t in_port_t;



#pragma GCC visibility push(default)
__uint32_t htonl(__uint32_t) __attribute__((__const__));
__uint16_t htons(__uint16_t) __attribute__((__const__));
__uint32_t ntohl(__uint32_t) __attribute__((__const__));
__uint16_t ntohs(__uint16_t) __attribute__((__const__));
#pragma GCC visibility pop





# 1 "C:/Users/VIC/gergios/sys\\machine/endian_machdep.h" 1




# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\endian_machdep.h" 1
# 6 "C:/Users/VIC/gergios/sys\\machine/endian_machdep.h" 2
# 79 "C:/Users/VIC/gergios/sys\\sys/endian.h" 2
# 107 "C:/Users/VIC/gergios/sys\\sys/endian.h"
# 1 "C:/Users/VIC/gergios/sys\\machine/bswap.h" 1
# 10 "C:/Users/VIC/gergios/sys\\machine/bswap.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/types.h" 1
# 11 "C:/Users/VIC/gergios/sys\\machine/bswap.h" 2


static __inline __uint16_t
bswap16(__uint16_t x)
{
 return __builtin_bswap16(x);
}

static __inline __uint32_t
bswap32(__uint32_t x)
{
 return __builtin_bswap32(x);
}

static __inline __uint64_t
bswap64(__uint64_t x)
{
 return __builtin_bswap64(x);
}
# 108 "C:/Users/VIC/gergios/sys\\sys/endian.h" 2
# 203 "C:/Users/VIC/gergios/sys\\sys/endian.h"
static __inline __attribute__((__unused__)) void be16enc(void *dst, __uint16_t u) { u = bswap16(((__uint16_t)((u)))); __builtin_memcpy(dst, &u, sizeof(u)); }
static __inline __attribute__((__unused__)) void be32enc(void *dst, __uint32_t u) { u = bswap32(((__uint32_t)((u)))); __builtin_memcpy(dst, &u, sizeof(u)); }
static __inline __attribute__((__unused__)) void be64enc(void *dst, __uint64_t u) { u = bswap64(((__uint64_t)((u)))); __builtin_memcpy(dst, &u, sizeof(u)); }
static __inline __attribute__((__unused__)) void le16enc(void *dst, __uint16_t u) { u = (u); __builtin_memcpy(dst, &u, sizeof(u)); }
static __inline __attribute__((__unused__)) void le32enc(void *dst, __uint32_t u) { u = (u); __builtin_memcpy(dst, &u, sizeof(u)); }
static __inline __attribute__((__unused__)) void le64enc(void *dst, __uint64_t u) { u = (u); __builtin_memcpy(dst, &u, sizeof(u)); }
# 220 "C:/Users/VIC/gergios/sys\\sys/endian.h"
static __inline __attribute__((__unused__)) __uint16_t be16dec(const void *buf) { __uint16_t u; __builtin_memcpy(&u, buf, sizeof(u)); return bswap16(((__uint16_t)((u)))); }
static __inline __attribute__((__unused__)) __uint32_t be32dec(const void *buf) { __uint32_t u; __builtin_memcpy(&u, buf, sizeof(u)); return bswap32(((__uint32_t)((u)))); }
static __inline __attribute__((__unused__)) __uint64_t be64dec(const void *buf) { __uint64_t u; __builtin_memcpy(&u, buf, sizeof(u)); return bswap64(((__uint64_t)((u)))); }
static __inline __attribute__((__unused__)) __uint16_t le16dec(const void *buf) { __uint16_t u; __builtin_memcpy(&u, buf, sizeof(u)); return (u); }
static __inline __attribute__((__unused__)) __uint32_t le32dec(const void *buf) { __uint32_t u; __builtin_memcpy(&u, buf, sizeof(u)); return (u); }
static __inline __attribute__((__unused__)) __uint64_t le64dec(const void *buf) { __uint64_t u; __builtin_memcpy(&u, buf, sizeof(u)); return (u); }
# 12 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\endian.h" 2
# 6 "C:/Users/VIC/gergios/sys\\machine/endian.h" 2
# 129 "C:/Users/VIC/gergios/sys\\sys/types.h" 2


typedef unsigned char u_char;
typedef unsigned short u_short;
typedef unsigned int u_int;
typedef unsigned long u_long;

typedef unsigned char unchar;
typedef unsigned short ushort;
typedef unsigned int uint;
typedef unsigned long ulong;


typedef __uint64_t u_quad_t;
typedef __int64_t quad_t;
typedef quad_t * qaddr_t;
# 156 "C:/Users/VIC/gergios/sys\\sys/types.h"
typedef __int64_t longlong_t;
typedef __uint64_t u_longlong_t;

typedef __int64_t blkcnt_t;
typedef __int32_t blksize_t;


typedef __fsblkcnt_t fsblkcnt_t;




typedef __fsfilcnt_t fsfilcnt_t;






typedef __caddr_t caddr_t;
# 184 "C:/Users/VIC/gergios/sys\\sys/types.h"
typedef __int64_t daddr_t;


typedef __uint64_t dev_t;
typedef __uint32_t fixpt_t;


typedef __gid_t gid_t;



typedef int idtype_t;
typedef __uint32_t id_t;
typedef __uint64_t ino_t;
typedef long key_t;


typedef __mode_t mode_t;



typedef __uint32_t nlink_t;


typedef __off_t off_t;




typedef __pid_t pid_t;


typedef __int32_t lwpid_t;
typedef __uint64_t rlim_t;
typedef __int32_t segsz_t;
typedef __int32_t swblk_t;


typedef __uid_t uid_t;



typedef int mqd_t;

typedef unsigned long cpuid_t;

typedef int psetid_t;

typedef volatile __cpu_simple_lock_nv_t __cpu_simple_lock_t;
# 275 "C:/Users/VIC/gergios/sys\\sys/types.h"
#pragma GCC visibility push(default)
__off_t lseek(int, __off_t, int);
int ftruncate(int, __off_t);
int truncate(const char *, __off_t);
#pragma GCC visibility pop






typedef __int32_t __devmajor_t, __devminor_t;
# 299 "C:/Users/VIC/gergios/sys\\sys/types.h"
typedef unsigned int clock_t;




typedef long int ptrdiff_t;




typedef long unsigned int size_t;





typedef long int ssize_t;




typedef __int64_t time_t;




typedef int clockid_t;




typedef int timer_t;




typedef int suseconds_t;




typedef unsigned int useconds_t;




# 1 "C:/Users/VIC/gergios/sys\\sys/fd_set.h" 1
# 38 "C:/Users/VIC/gergios/sys\\sys/fd_set.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 39 "C:/Users/VIC/gergios/sys\\sys/fd_set.h" 2







typedef __uint32_t __fd_mask;
# 68 "C:/Users/VIC/gergios/sys\\sys/fd_set.h"
typedef struct fd_set {
 __fd_mask fds_bits[(((255) + ((32) - 1)) / (32))];
} fd_set;
# 346 "C:/Users/VIC/gergios/sys\\sys/types.h" 2



typedef struct kauth_cred *kauth_cred_t;

typedef int pri_t;
# 5 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2


# 1 "C:/Users/VIC/gergios/sys\\machine/multiboot.h" 1
# 8 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2


# 1 "C:/Users/VIC/gergios/minix/include\\minix/sys_config.h" 1
# 11 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2


# 1 "C:/Users/VIC/gergios/sys\\sys/sigtypes.h" 1
# 48 "C:/Users/VIC/gergios/sys\\sys/sigtypes.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 49 "C:/Users/VIC/gergios/sys\\sys/sigtypes.h" 2
# 60 "C:/Users/VIC/gergios/sys\\sys/sigtypes.h"
typedef struct {
 __uint32_t __bits[4];
} sigset_t;
# 108 "C:/Users/VIC/gergios/sys\\sys/sigtypes.h"
typedef struct

               sigaltstack

      {
 void *ss_sp;
 size_t ss_size;
 int ss_flags;
} stack_t;
# 14 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2

# 1 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 1 3
# 100 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 3
typedef long int __int64_t;

typedef long unsigned int __uint64_t;
# 122 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 3
typedef __int64_t int_least64_t;
typedef __uint64_t uint_least64_t;
typedef __int64_t int_fast64_t;
typedef __uint64_t uint_fast64_t;
# 197 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 3
typedef int __int32_t;




typedef unsigned int __uint32_t;
# 220 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 3
typedef __int32_t int_least32_t;
typedef __uint32_t uint_least32_t;
typedef __int32_t int_fast32_t;
typedef __uint32_t uint_fast32_t;
# 245 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 3
typedef short __int16_t;

typedef unsigned short __uint16_t;
# 259 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 3
typedef __int16_t int_least16_t;
typedef __uint16_t uint_least16_t;
typedef __int16_t int_fast16_t;
typedef __uint16_t uint_fast16_t;





typedef signed char __int8_t;

typedef unsigned char __uint8_t;







typedef __int8_t int_least8_t;
typedef __uint8_t uint_least8_t;
typedef __int8_t int_fast8_t;
typedef __uint8_t uint_fast8_t;
# 295 "C:\\Program Files\\LLVM\\lib\\clang\\21\\include\\stdint.h" 3
typedef long int intptr_t;






typedef long unsigned int uintptr_t;





typedef long int intmax_t;
typedef long unsigned int uintmax_t;
# 16 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2
# 1 "C:/Users/VIC/gergios/include\\stddef.h" 1
# 38 "C:/Users/VIC/gergios/include\\stddef.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 39 "C:/Users/VIC/gergios/include\\stddef.h" 2
# 52 "C:/Users/VIC/gergios/include\\stddef.h"
typedef unsigned int wchar_t;



# 1 "C:/Users/VIC/gergios/sys\\sys/null.h" 1
# 57 "C:/Users/VIC/gergios/include\\stddef.h" 2
# 17 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2


typedef unsigned int vir_clicks;
typedef unsigned long phys_bytes;
typedef unsigned int phys_clicks;
typedef int endpoint_t;
typedef __int32_t cp_grant_id_t;
typedef long unsigned int vir_bytes;


struct vir_addr {
  endpoint_t proc_nr_e;
  vir_bytes offset;
};


struct vir_cp_req {
  struct vir_addr src;
  struct vir_addr dst;
  phys_bytes count;
};


struct vumap_vir {
  union {
 cp_grant_id_t u_grant;
 vir_bytes u_addr;
  } vv_u;
  size_t vv_size;
};



struct vumap_phys {
  phys_bytes vp_addr;
  size_t vp_size;
};


typedef struct {
  vir_bytes iov_addr;
  vir_bytes iov_size;
} iovec_t;

typedef struct {
  cp_grant_id_t iov_grant;
  vir_bytes iov_size;
} iovec_s_t;






struct sigmsg {
  int sm_signo;
  sigset_t sm_mask;
  vir_bytes sm_sighandler;
  vir_bytes sm_sigreturn;
  vir_bytes sm_stkptr;
};


struct cpuavg {
 clock_t ca_base;
 __uint32_t ca_run;
 __uint32_t ca_last;
 __uint32_t ca_avg;
};
# 98 "C:/Users/VIC/gergios/minix/include\\minix/type.h"
struct loadinfo {
  __uint16_t proc_load_history[((60*15)/6)];
  __uint16_t proc_last_slot;
  clock_t last_clock;
};

struct kclockinfo {
  time_t boottime;

  clock_t uptime;
  __uint32_t _rsvd1;
  clock_t realtime;
  __uint32_t _rsvd2;
# 119 "C:/Users/VIC/gergios/minix/include\\minix/type.h"
  __uint32_t hz;
};

struct machine {
  unsigned processors_count;
  unsigned bsp_id;
  int padding;
  int apic_enabled;
  phys_bytes acpi_rsdp;
  unsigned int board_id;


};

struct io_range
{
 unsigned ior_base;
 unsigned ior_limit;
};

struct minix_mem_range
{
 phys_bytes mr_base;
 phys_bytes mr_limit;
};




struct boot_image {
  int proc_nr;
  char proc_name[16];
  endpoint_t endpoint;
  phys_bytes start_addr;
  phys_bytes len;
};


struct memory {
 phys_bytes base;
 phys_bytes size;
};
# 170 "C:/Users/VIC/gergios/minix/include\\minix/type.h"
struct kmessages {
  int km_next;
  int km_size;
  char km_buf[10000];
  char kmess_buf[80*25];
  int blpos;
};

# 1 "C:/Users/VIC/gergios/minix/include\\minix/config.h" 1
# 179 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2
# 1 "C:/Users/VIC/gergios/sys\\machine/interrupt.h" 1
# 180 "C:/Users/VIC/gergios/minix/include\\minix/type.h" 2





typedef unsigned short rand_t;

struct k_randomness {
  int random_elements, random_sources;
  struct k_randomness_bin {
        int r_next;
        int r_size;
        rand_t r_buf[64];
  } bin[16];
};


struct arm_frclock {
 __uint64_t hz;
 __uint32_t tcrr;
};




struct kuserinfo {
 size_t kui_size;
 vir_bytes kui_user_sp;
};





struct minix_kerninfo {
# 230 "C:/Users/VIC/gergios/minix/include\\minix/type.h"
 __uint32_t kerninfo_magic;
 __uint32_t minix_feature_flags;
 __uint32_t ki_flags;
 __uint32_t flags_unused2;
 __uint32_t flags_unused3;
 __uint32_t flags_unused4;
 struct kinfo *kinfo;
 struct machine *machine;
 struct kmessages *kmessages;
 struct loadinfo *loadinfo;
 struct minix_ipcvecs *minix_ipcvecs;
 struct kuserinfo *kuserinfo;
 struct arm_frclock *arm_frclock;
 volatile struct kclockinfo *kclockinfo;
};
# 6 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h" 2
# 1 "C:/Users/VIC/gergios/minix/include\\minix/const.h" 1



# 1 "C:/Users/VIC/gergios/sys\\machine/archconst.h" 1
# 5 "C:/Users/VIC/gergios/minix/include\\minix/const.h" 2
# 7 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h" 2
# 1 "C:/Users/VIC/gergios/sys\\sys/signal.h" 1
# 42 "C:/Users/VIC/gergios/sys\\sys/signal.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 43 "C:/Users/VIC/gergios/sys\\sys/signal.h" 2
# 117 "C:/Users/VIC/gergios/sys\\sys/signal.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/siginfo.h" 1
# 35 "C:/Users/VIC/gergios/sys\\sys/siginfo.h"
# 1 "C:/Users/VIC/gergios/sys\\machine/signal.h" 1




# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\signal.h" 1
# 20 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\signal.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 21 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\signal.h" 2


typedef int sig_atomic_t;
# 51 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\signal.h"
struct sigcontext {

 int sc_onstack;
 int __sc_mask13;


 __uint64_t sc_faultaddr;


 __uint64_t sc_x[31];


 __uint64_t sc_sp;
 __uint64_t sc_pc;
 __uint64_t sc_pstate;


 sigset_t sc_mask;


 int sc_magic;
 int sc_flags;
 int trap_style;
};
# 104 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\signal.h"
#pragma GCC visibility push(default)
int sigreturn(struct sigcontext *_scp);
#pragma GCC visibility pop
# 6 "C:/Users/VIC/gergios/sys\\machine/signal.h" 2
# 36 "C:/Users/VIC/gergios/sys\\sys/siginfo.h" 2
# 1 "C:/Users/VIC/gergios/sys\\sys/featuretest.h" 1
# 37 "C:/Users/VIC/gergios/sys\\sys/siginfo.h" 2




typedef union sigval {
 int sival_int;
 void *sival_ptr;
} sigval_t;

struct _ksiginfo {
 int _signo;
 int _code;
 int _errno;


 int _pad;

 union {
  struct {
   __pid_t _pid;
   __uid_t _uid;
   sigval_t _value;
  } _rt;

  struct {
   __pid_t _pid;
   __uid_t _uid;
   int _status;
   clock_t _utime;
   clock_t _stime;
  } _child;

  struct {
   void *_addr;
   int _trap;
   int _trap2;
   int _trap3;
  } _fault;

  struct {
   long _band;
   int _fd;
  } _poll;
 } _reason;
};
# 133 "C:/Users/VIC/gergios/sys\\sys/siginfo.h"
typedef union siginfo {
 char si_pad[128];
 struct _ksiginfo _info;
} siginfo_t;
# 118 "C:/Users/VIC/gergios/sys\\sys/signal.h" 2




# 1 "C:/Users/VIC/gergios/sys\\sys/ucontext.h" 1
# 36 "C:/Users/VIC/gergios/sys\\sys/ucontext.h"
# 1 "C:/Users/VIC/gergios/sys\\machine/mcontext.h" 1




# 1 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\mcontext.h" 1
# 26 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\mcontext.h"
# 1 "C:/Users/VIC/gergios/sys\\sys/stdint.h" 1
# 79 "C:/Users/VIC/gergios/sys\\sys/stdint.h"
typedef __intptr_t intptr_t;




typedef __uintptr_t uintptr_t;



# 1 "C:/Users/VIC/gergios/sys\\machine/int_mwgwtypes.h" 1
# 89 "C:/Users/VIC/gergios/sys\\sys/stdint.h" 2



# 1 "C:/Users/VIC/gergios/sys\\machine/int_limits.h" 1
# 93 "C:/Users/VIC/gergios/sys\\sys/stdint.h" 2




# 1 "C:/Users/VIC/gergios/sys\\machine/int_const.h" 1
# 98 "C:/Users/VIC/gergios/sys\\sys/stdint.h" 2


# 1 "C:/Users/VIC/gergios/sys\\machine/wchar_limits.h" 1
# 101 "C:/Users/VIC/gergios/sys\\sys/stdint.h" 2
# 27 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\mcontext.h" 2





typedef __uint64_t __greg_t;
typedef __greg_t __gregset_t[31];
# 88 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\mcontext.h"
typedef struct __fpreg {
 __uint64_t fp_low;
 __uint64_t fp_high;
} __fpreg_t;



typedef struct __fpregset {
 __uint64_t fp_fpsr;
 __uint64_t fp_fpcr;
 __fpreg_t fp_reg[32];
} __fpregset_t;
# 113 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\mcontext.h"
typedef struct {

 __gregset_t __gregs;


 __uint64_t __sp;
 __uint64_t __pc;
 __uint64_t __pstate;


 __fpregset_t __fpregs;


 int mc_flags;
 int mc_magic;
} mcontext_t;
# 188 "C:/Users/VIC/gergios/sys/arch/aarch64/include\\mcontext.h"
#pragma GCC visibility push(default)


int setmcontext(const mcontext_t *mcp);
int getmcontext(mcontext_t *mcp);

#pragma GCC visibility pop
# 6 "C:/Users/VIC/gergios/sys\\machine/mcontext.h" 2
# 37 "C:/Users/VIC/gergios/sys\\sys/ucontext.h" 2

typedef struct __ucontext ucontext_t;

struct __ucontext {
 unsigned int uc_flags;
 ucontext_t * uc_link;
 sigset_t uc_sigmask;
 stack_t uc_stack;
 mcontext_t uc_mcontext;

 long __uc_pad[1];

};
# 124 "C:/Users/VIC/gergios/sys\\sys/ucontext.h"
#pragma GCC visibility push(default)
void resumecontext(ucontext_t *ucp);



int getuctx(ucontext_t *ucp);
int setuctx(const ucontext_t *ucp);
#pragma GCC visibility pop
# 123 "C:/Users/VIC/gergios/sys\\sys/signal.h" 2





struct sigaction {
 union {
  void (*_sa_handler)(int);


  void (*_sa_sigaction)(int, siginfo_t *, void *);

 } _sa_u;
 sigset_t sa_mask;
 int sa_flags;
};
# 183 "C:/Users/VIC/gergios/sys\\sys/signal.h"
typedef void (*sig_t)(int);
# 205 "C:/Users/VIC/gergios/sys\\sys/signal.h"
struct sigstack {
 void *ss_sp;
 int ss_onstack;
};
# 223 "C:/Users/VIC/gergios/sys\\sys/signal.h"
struct sigevent {
 int sigev_notify;
 int sigev_signo;
 union sigval sigev_value;
 void (*sigev_notify_function)(union sigval);
 void *sigev_notify_attributes;
};
# 245 "C:/Users/VIC/gergios/sys\\sys/signal.h"
#pragma GCC visibility push(default)
void (*signal(int, void (*)(int)))(int);

int sigqueue(__pid_t, int, const union sigval);


int sigqueueinfo(__pid_t, const siginfo_t *);

#pragma GCC visibility pop
# 8 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h" 2
# 17 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef struct {
 __uint8_t data[56];
} mess_u8;
typedef int _ASSERT_mess_u8[ sizeof(mess_u8) <= 64 ? 1 : -1];

typedef struct {
 __uint16_t data[28];
} mess_u16;
typedef int _ASSERT_mess_u16[ sizeof(mess_u16) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t data[14];
} mess_u32;
typedef int _ASSERT_mess_u32[ sizeof(mess_u32) <= 64 ? 1 : -1];

typedef struct {
 __uint64_t data[7];
} mess_u64;
typedef int _ASSERT_mess_u64[ sizeof(mess_u64) <= 64 ? 1 : -1];

typedef struct {
 __uint64_t m1ull1;
 int m1i1, m1i2, m1i3;
 char *m1p1, *m1p2, *m1p3, *m1p4;





} mess_1;
typedef int _ASSERT_mess_1[ sizeof(mess_1) <= 64 ? 1 : -1];

typedef struct {
 __int64_t m2ll1;
 int m2i1, m2i2, m2i3;

 int m2l1, m2l2;



 char *m2p1;
 sigset_t sigset;
 short m2s1;





} mess_2;
typedef int _ASSERT_mess_2[ sizeof(mess_2) <= 64 ? 1 : -1];

typedef struct {
 int m3i1, m3i2;
 char *m3p1;
 char m3ca1[44];
} mess_3;
typedef int _ASSERT_mess_3[ sizeof(mess_3) <= 64 ? 1 : -1];

typedef struct {
 __int64_t m4ll1;
 long m4l1, m4l2, m4l3, m4l4, m4l5;





} mess_4;
typedef int _ASSERT_mess_4[ sizeof(mess_4) <= 64 ? 1 : -1];

typedef struct {
 int m7i1, m7i2, m7i3, m7i4, m7i5;
 char *m7p1, *m7p2;





} mess_7;
typedef int _ASSERT_mess_7[ sizeof(mess_7) <= 64 ? 1 : -1];

typedef struct {
 __uint64_t m9ull1, m9ull2;
 long m9l1, m9l2, m9l3, m9l4, m9l5;
 short m9s1, m9s2, m9s3, m9s4;





} mess_9;
typedef int _ASSERT_mess_9[ sizeof(mess_9) <= 64 ? 1 : -1];

typedef struct {
 __uint64_t m10ull1;
 int m10i1, m10i2, m10i3, m10i4;
 long m10l1, m10l2, m10l3;





} mess_10;
typedef int _ASSERT_mess_10[ sizeof(mess_10) <= 64 ? 1 : -1];


union ds_val {
 cp_grant_id_t grant;
 __uint32_t u32;
 endpoint_t ep;
};

typedef struct {
 union ds_val val_out;
 int val_len;
 __uint8_t padding[48];
} mess_ds_reply;
typedef int _ASSERT_mess_ds_reply[ sizeof(mess_ds_reply) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t key_grant;
 int key_len;
 int flags;
 union ds_val val_in;
 int val_len;
 endpoint_t owner;
 __uint8_t padding[32];
} mess_ds_req;
typedef int _ASSERT_mess_ds_req[ sizeof(mess_ds_req) <= 64 ? 1 : -1];

typedef struct {
 __off_t seek_pos;

 size_t nbytes;

 __uint8_t data[44];
} mess_fs_vfs_breadwrite;
typedef int _ASSERT_mess_fs_vfs_breadwrite[ sizeof(mess_fs_vfs_breadwrite) <= 64 ? 1 : -1];

typedef struct {
 __mode_t mode;

 __uint8_t data[52];
} mess_fs_vfs_chmod;
typedef int _ASSERT_mess_fs_vfs_chmod[ sizeof(mess_fs_vfs_chmod) <= 64 ? 1 : -1];

typedef struct {
 __mode_t mode;

 __uint8_t data[52];
} mess_fs_vfs_chown;
typedef int _ASSERT_mess_fs_vfs_chown[ sizeof(mess_fs_vfs_chown) <= 64 ? 1 : -1];

typedef struct {
 __off_t file_size;
 ino_t inode;

 __mode_t mode;
 __uid_t uid;
 __gid_t gid;

 __uint8_t data[28];
} mess_fs_vfs_create;
typedef int _ASSERT_mess_fs_vfs_create[ sizeof(mess_fs_vfs_create) <= 64 ? 1 : -1];

typedef struct {
 __off_t seek_pos;

 size_t nbytes;

 __uint8_t data[44];
} mess_fs_vfs_getdents;
typedef int _ASSERT_mess_fs_vfs_getdents[ sizeof(mess_fs_vfs_getdents) <= 64 ? 1 : -1];

typedef struct {
 __off_t offset;
 __off_t file_size;
 dev_t device;
 ino_t inode;

 __mode_t mode;
 __uid_t uid;
 __gid_t gid;

 __uint16_t symloop;

 __uint8_t data[10];
} mess_fs_vfs_lookup;
typedef int _ASSERT_mess_fs_vfs_lookup[ sizeof(mess_fs_vfs_lookup) <= 64 ? 1 : -1];

typedef struct {
 __off_t file_size;
 dev_t device;
 ino_t inode;

 __mode_t mode;
 __uid_t uid;
 __gid_t gid;

 __uint8_t data[20];
} mess_fs_vfs_newnode;
typedef int _ASSERT_mess_fs_vfs_newnode[ sizeof(mess_fs_vfs_newnode) <= 64 ? 1 : -1];

typedef struct {
 size_t nbytes;

 __uint8_t data[52];
} mess_fs_vfs_rdlink;
typedef int _ASSERT_mess_fs_vfs_rdlink[ sizeof(mess_fs_vfs_rdlink) <= 64 ? 1 : -1];

typedef struct {
 __off_t file_size;
 dev_t device;
 ino_t inode;

 __uint32_t flags;
 __mode_t mode;
 __uid_t uid;
 __gid_t gid;

 __uint16_t con_reqs;

 __uint8_t data[14];
} mess_fs_vfs_readsuper;
typedef int _ASSERT_mess_fs_vfs_readsuper[ sizeof(mess_fs_vfs_readsuper) <= 64 ? 1 : -1];

typedef struct {
 __off_t seek_pos;

 size_t nbytes;

 __uint8_t data[44];
} mess_fs_vfs_readwrite;
typedef int _ASSERT_mess_fs_vfs_readwrite[ sizeof(mess_fs_vfs_readwrite) <= 64 ? 1 : -1];

typedef struct {
 __uint8_t padding[56];
} mess_i2c_li2cdriver_busc_i2c_exec;
typedef int _ASSERT_mess_i2c_li2cdriver_busc_i2c_exec[ sizeof(mess_i2c_li2cdriver_busc_i2c_exec) <= 64 ? 1 : -1];

typedef struct {
 __uint8_t padding[56];
} mess_i2c_li2cdriver_busc_i2c_reserve;
typedef int _ASSERT_mess_i2c_li2cdriver_busc_i2c_reserve[ sizeof(mess_i2c_li2cdriver_busc_i2c_reserve) <= 64 ? 1 : -1];

typedef struct {
 int kbd_id;
 int mouse_id;
 int rsvd1_id;
 int rsvd2_id;

 __uint8_t padding[40];
} mess_input_linputdriver_input_conf;
typedef int _ASSERT_mess_input_linputdriver_input_conf[ sizeof(mess_input_linputdriver_input_conf) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t led_mask;

 __uint8_t padding[52];
} mess_input_linputdriver_setleds;
typedef int _ASSERT_mess_input_linputdriver_setleds[ sizeof(mess_input_linputdriver_setleds) <= 64 ? 1 : -1];

typedef struct {
 int id;
 int page;
 int code;
 int value;
 int flags;

 __uint8_t padding[36];
} mess_input_tty_event;
typedef int _ASSERT_mess_input_tty_event[ sizeof(mess_input_tty_event) <= 64 ? 1 : -1];

typedef struct {
 time_t acnt_queue;

 unsigned long acnt_deqs;
 unsigned long acnt_ipc_sync;
 unsigned long acnt_ipc_async;
 unsigned long acnt_preempt;
 __uint32_t acnt_cpu;
 __uint32_t acnt_cpu_load;






} mess_krn_lsys_schedule;
typedef int _ASSERT_mess_krn_lsys_schedule[ sizeof(mess_krn_lsys_schedule) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t value;

 __uint8_t padding[52];
} mess_krn_lsys_sys_devio;
typedef int _ASSERT_mess_krn_lsys_sys_devio[ sizeof(mess_krn_lsys_sys_devio) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes msgaddr;

 __uint8_t padding[48];
} mess_krn_lsys_sys_fork;
typedef int _ASSERT_mess_krn_lsys_sys_fork[ sizeof(mess_krn_lsys_sys_fork) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 int privflags;
 int initflags;
 char name[44];

} mess_krn_lsys_sys_getwhoami;
typedef int _ASSERT_mess_krn_lsys_sys_getwhoami[ sizeof(mess_krn_lsys_sys_getwhoami) <= 64 ? 1 : -1];

typedef struct {
 int hook_id;

 __uint8_t padding[52];
} mess_krn_lsys_sys_irqctl;
typedef int _ASSERT_mess_krn_lsys_sys_irqctl[ sizeof(mess_krn_lsys_sys_irqctl) <= 64 ? 1 : -1];

typedef struct {
 clock_t real_ticks;
 clock_t boot_ticks;
 clock_t user_time;
 clock_t system_time;
 time_t boot_time;

 __uint8_t padding[32];
} mess_krn_lsys_sys_times;
typedef int _ASSERT_mess_krn_lsys_sys_times[ sizeof(mess_krn_lsys_sys_times) <= 64 ? 1 : -1];

typedef struct {
 long int data;

 __uint8_t padding[52];
} mess_krn_lsys_sys_trace;
typedef int _ASSERT_mess_krn_lsys_sys_trace[ sizeof(mess_krn_lsys_sys_trace) <= 64 ? 1 : -1];

typedef struct {
 phys_bytes dst_addr;

 __uint8_t padding[52];
} mess_krn_lsys_sys_umap;
typedef int _ASSERT_mess_krn_lsys_sys_umap[ sizeof(mess_krn_lsys_sys_umap) <= 64 ? 1 : -1];

typedef struct {
 int pcount;

 __uint8_t padding[52];
} mess_krn_lsys_sys_vumap;
typedef int _ASSERT_mess_krn_lsys_sys_vumap[ sizeof(mess_krn_lsys_sys_vumap) <= 64 ? 1 : -1];

typedef struct {
 __off_t pos;

 int minor;
 int id;
 int access;

 int count;
 cp_grant_id_t grant;
 int flags;

 endpoint_t user;
 unsigned long request;

 __uint8_t padding[16];
} mess_lbdev_lblockdriver_msg;
typedef int _ASSERT_mess_lbdev_lblockdriver_msg[ sizeof(mess_lbdev_lblockdriver_msg) <= 64 ? 1 : -1];

typedef struct {
 int status;
 int id;

 __uint8_t padding[48];
} mess_lblockdriver_lbdev_reply;
typedef int _ASSERT_mess_lblockdriver_lbdev_reply[ sizeof(mess_lblockdriver_lbdev_reply) <= 64 ? 1 : -1];

typedef struct {
 int id;
 int num;
 int cmd;
 vir_bytes opt;
 int ret;
 __uint8_t padding[36];
} mess_lc_ipc_semctl;
typedef int _ASSERT_mess_lc_ipc_semctl[ sizeof(mess_lc_ipc_semctl) <= 64 ? 1 : -1];

typedef struct {
 key_t key;
 int nr;
 int flag;
 int retid;
 __uint8_t padding[40];
} mess_lc_ipc_semget;
typedef int _ASSERT_mess_lc_ipc_semget[ sizeof(mess_lc_ipc_semget) <= 64 ? 1 : -1];

typedef struct {
 int id;
 void *ops;
 unsigned int size;
 __uint8_t padding[42];
} mess_lc_ipc_semop;
typedef int _ASSERT_mess_lc_ipc_semop[ sizeof(mess_lc_ipc_semop) <= 64 ? 1 : -1];

typedef struct {
 int id;
 const void *addr;
 int flag; void *retaddr;

 __uint8_t padding[32];



} mess_lc_ipc_shmat;
typedef int _ASSERT_mess_lc_ipc_shmat[ sizeof(mess_lc_ipc_shmat) <= 64 ? 1 : -1];

typedef struct {
 int id;
 int cmd;
 void *buf;
 int ret;
 __uint8_t padding[40];
} mess_lc_ipc_shmctl;
typedef int _ASSERT_mess_lc_ipc_shmctl[ sizeof(mess_lc_ipc_shmctl) <= 64 ? 1 : -1];

typedef struct {
 const void *addr;
 __uint8_t padding[52];
} mess_lc_ipc_shmdt;
typedef int _ASSERT_mess_lc_ipc_shmdt[ sizeof(mess_lc_ipc_shmdt) <= 64 ? 1 : -1];

typedef struct {
 key_t key;
 size_t size;
 int flag;
 int retid;
 __uint8_t padding[40];
} mess_lc_ipc_shmget;
typedef int _ASSERT_mess_lc_ipc_shmget[ sizeof(mess_lc_ipc_shmget) <= 64 ? 1 : -1];


typedef struct {
 vir_bytes oldp;
 vir_bytes newp;
 __uint32_t oldlen;
 __uint32_t newlen;
 __uint32_t namelen;
 vir_bytes namep;
 int name[6];
} mess_lc_mib_sysctl;
# 480 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef int _ASSERT_mess_lc_mib_sysctl[ sizeof(mess_lc_mib_sysctl) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes name;
 size_t namelen;
 vir_bytes frame;
 size_t framelen;
 vir_bytes ps_str;


 __uint8_t padding[24];



} mess_lc_pm_exec;
typedef int _ASSERT_mess_lc_pm_exec[ sizeof(mess_lc_pm_exec) <= 64 ? 1 : -1];

typedef struct {
 int status;

 __uint8_t padding[52];
} mess_lc_pm_exit;
typedef int _ASSERT_mess_lc_pm_exit[ sizeof(mess_lc_pm_exit) <= 64 ? 1 : -1];

typedef struct {
 __pid_t pid;

 __uint8_t padding[52];
} mess_lc_pm_getsid;
typedef int _ASSERT_mess_lc_pm_getsid[ sizeof(mess_lc_pm_getsid) <= 64 ? 1 : -1];

typedef struct {
 int num;
 vir_bytes ptr;

 __uint8_t padding[48];
} mess_lc_pm_groups;
typedef int _ASSERT_mess_lc_pm_groups[ sizeof(mess_lc_pm_groups) <= 64 ? 1 : -1];

typedef struct {
 int which;
 vir_bytes value;
 vir_bytes ovalue;


 __uint8_t padding[40];



} mess_lc_pm_itimer;
typedef int _ASSERT_mess_lc_pm_itimer[ sizeof(mess_lc_pm_itimer) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes ctx;

 __uint8_t padding[52];
} mess_lc_pm_mcontext;
typedef int _ASSERT_mess_lc_pm_mcontext[ sizeof(mess_lc_pm_mcontext) <= 64 ? 1 : -1];

typedef struct {
 int which;
 int who;
 int prio;

 __uint8_t padding[44];
} mess_lc_pm_priority;
typedef int _ASSERT_mess_lc_pm_priority[ sizeof(mess_lc_pm_priority) <= 64 ? 1 : -1];

typedef struct {
 __pid_t pid;
 int req;
 vir_bytes addr;
 long data;

 __uint8_t padding[40];
} mess_lc_pm_ptrace;
typedef int _ASSERT_mess_lc_pm_ptrace[ sizeof(mess_lc_pm_ptrace) <= 64 ? 1 : -1];

typedef struct {
 int how;

 __uint8_t padding[52];
} mess_lc_pm_reboot;
typedef int _ASSERT_mess_lc_pm_reboot[ sizeof(mess_lc_pm_reboot) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t who;
 vir_bytes addr;

 __uint8_t padding[48];
} mess_lc_pm_rusage;
typedef int _ASSERT_mess_lc_pm_rusage[ sizeof(mess_lc_pm_rusage) <= 64 ? 1 : -1];

typedef struct {
 __gid_t gid;

 __uint8_t padding[52];
} mess_lc_pm_setgid;
typedef int _ASSERT_mess_lc_pm_setgid[ sizeof(mess_lc_pm_setgid) <= 64 ? 1 : -1];

typedef struct {
 __uid_t uid;

 __uint8_t padding[52];
} mess_lc_pm_setuid;
typedef int _ASSERT_mess_lc_pm_setuid[ sizeof(mess_lc_pm_setuid) <= 64 ? 1 : -1];

typedef struct {
 __pid_t pid;
 int nr;
 vir_bytes act;
 vir_bytes oact;
 vir_bytes ret;


 __uint8_t padding[32];



} mess_lc_pm_sig;
typedef int _ASSERT_mess_lc_pm_sig[ sizeof(mess_lc_pm_sig) <= 64 ? 1 : -1];

typedef struct {
 int how;
 vir_bytes ctx;
 sigset_t set;

 __uint8_t padding[32];
} mess_lc_pm_sigset;
typedef int _ASSERT_mess_lc_pm_sigset[ sizeof(mess_lc_pm_sigset) <= 64 ? 1 : -1];

typedef struct {
 int action;
 int freq;
 int intr_type;
 vir_bytes ctl_ptr;
 vir_bytes mem_ptr;
 size_t mem_size;


 __uint8_t padding[24];



} mess_lc_pm_sprof;
typedef int _ASSERT_mess_lc_pm_sprof[ sizeof(mess_lc_pm_sprof) <= 64 ? 1 : -1];

typedef struct {
 int req;
 int field;
 size_t len;
 vir_bytes value;

 __uint8_t padding[40];
} mess_lc_pm_sysuname;
typedef int _ASSERT_mess_lc_pm_sysuname[ sizeof(mess_lc_pm_sysuname) <= 64 ? 1 : -1];

typedef struct {
 time_t sec;

 clockid_t clk_id;
 int now;
 long nsec;

 __uint8_t padding[36];
} mess_lc_pm_time;
typedef int _ASSERT_mess_lc_pm_time[ sizeof(mess_lc_pm_time) <= 64 ? 1 : -1];

typedef struct {
 __pid_t pid;
 int options;
 vir_bytes addr;

 __uint8_t padding[44];
} mess_lc_pm_wait4;
typedef int _ASSERT_mess_lc_pm_wait4[ sizeof(mess_lc_pm_wait4) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t grant;
 vir_bytes tm;
 int flags;

 __uint8_t padding[44];
} mess_lc_readclock_rtcdev;
typedef int _ASSERT_mess_lc_readclock_rtcdev[ sizeof(mess_lc_readclock_rtcdev) <= 64 ? 1 : -1];

typedef struct {
 unsigned long request;
 vir_bytes arg;

 __uint8_t padding[48];
} mess_lc_svrctl;
typedef int _ASSERT_mess_lc_svrctl[ sizeof(mess_lc_svrctl) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes name;
 size_t len;
 int fd;
 __uid_t owner;
 __gid_t group;

 __uint8_t padding[36];
} mess_lc_vfs_chown;
typedef int _ASSERT_mess_lc_vfs_chown[ sizeof(mess_lc_vfs_chown) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 int nblock;

 __uint8_t padding[48];
} mess_lc_vfs_close;
typedef int _ASSERT_mess_lc_vfs_close[ sizeof(mess_lc_vfs_close) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes name;
 size_t len;
 int flags;
 __mode_t mode;

 __uint8_t padding[40];
} mess_lc_vfs_creat;
typedef int _ASSERT_mess_lc_vfs_creat[ sizeof(mess_lc_vfs_creat) <= 64 ? 1 : -1];

typedef struct {
 int fd;

 __uint8_t padding[52];
} mess_lc_vfs_fchdir;
typedef int _ASSERT_mess_lc_vfs_fchdir[ sizeof(mess_lc_vfs_fchdir) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 __mode_t mode;

 __uint8_t padding[48];
} mess_lc_vfs_fchmod;
typedef int _ASSERT_mess_lc_vfs_fchmod[ sizeof(mess_lc_vfs_fchmod) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 int cmd;
 int arg_int;
 vir_bytes arg_ptr;

 __uint8_t padding[40];
} mess_lc_vfs_fcntl;
typedef int _ASSERT_mess_lc_vfs_fcntl[ sizeof(mess_lc_vfs_fcntl) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 vir_bytes buf;

 __uint8_t padding[48];
} mess_lc_vfs_fstat;
typedef int _ASSERT_mess_lc_vfs_fstat[ sizeof(mess_lc_vfs_fstat) <= 64 ? 1 : -1];

typedef struct {
 int fd;

 __uint8_t padding[52];
} mess_lc_vfs_fsync;
typedef int _ASSERT_mess_lc_vfs_fsync[ sizeof(mess_lc_vfs_fsync) <= 64 ? 1 : -1];

typedef struct {
 size_t labellen;
 size_t buflen;
 vir_bytes label;
 vir_bytes buf;


 __uint8_t padding[32];



} mess_lc_vfs_gcov;
typedef int _ASSERT_mess_lc_vfs_gcov[ sizeof(mess_lc_vfs_gcov) <= 64 ? 1 : -1];

typedef struct {
 __int32_t flags;
 size_t len;
 vir_bytes buf;


 __uint8_t padding[40];



} mess_lc_vfs_getvfsstat;
typedef int _ASSERT_mess_lc_vfs_getvfsstat[ sizeof(mess_lc_vfs_getvfsstat) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 unsigned long req;
 vir_bytes arg;


 __uint8_t padding[40];



} mess_lc_vfs_ioctl;
typedef int _ASSERT_mess_lc_vfs_ioctl[ sizeof(mess_lc_vfs_ioctl) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes name1;
 vir_bytes name2;
 size_t len1;
 size_t len2;


 __uint8_t padding[32];



} mess_lc_vfs_link;
typedef int _ASSERT_mess_lc_vfs_link[ sizeof(mess_lc_vfs_link) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 int backlog;

 __uint8_t padding[48];
} mess_lc_vfs_listen;
typedef int _ASSERT_mess_lc_vfs_listen[ sizeof(mess_lc_vfs_listen) <= 64 ? 1 : -1];

typedef struct {
 __off_t offset;

 int fd;
 int whence;

 __uint8_t padding[40];
} mess_lc_vfs_lseek;
typedef int _ASSERT_mess_lc_vfs_lseek[ sizeof(mess_lc_vfs_lseek) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;

 vir_bytes name;
 size_t len;
 __mode_t mode;

 __uint8_t padding[36];
} mess_lc_vfs_mknod;
typedef int _ASSERT_mess_lc_vfs_mknod[ sizeof(mess_lc_vfs_mknod) <= 64 ? 1 : -1];


typedef struct {
 int flags;
 __uint32_t devlen;
 __uint32_t pathlen;
 __uint32_t typelen;
 __uint32_t labellen;
 vir_bytes dev;
 vir_bytes path;
 vir_bytes type;
 vir_bytes label;

 __uint8_t padding[8];
} mess_lc_vfs_mount;
# 855 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef int _ASSERT_mess_lc_vfs_mount[ sizeof(mess_lc_vfs_mount) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes name;
 size_t len;
 int flags;
 __mode_t mode;
 char buf[40];
} mess_lc_vfs_path;
typedef int _ASSERT_mess_lc_vfs_path[ sizeof(mess_lc_vfs_path) <= 64 ? 1 : -1];

typedef struct {





 int flags;
 int _unused;
 int oflags;

 __uint8_t padding[44];
} mess_lc_vfs_pipe2;
typedef int _ASSERT_mess_lc_vfs_pipe2[ sizeof(mess_lc_vfs_pipe2) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes name;
 size_t namelen;
 vir_bytes buf;
 size_t bufsize;


 __uint8_t padding[32];



} mess_lc_vfs_readlink;
typedef int _ASSERT_mess_lc_vfs_readlink[ sizeof(mess_lc_vfs_readlink) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 vir_bytes buf;
 size_t len;
 size_t cum_io;


 __uint8_t padding[32];



} mess_lc_vfs_readwrite;
typedef int _ASSERT_mess_lc_vfs_readwrite[ sizeof(mess_lc_vfs_readwrite) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t nfds;
 fd_set *readfds;
 fd_set *writefds;
 fd_set *errorfds;
 vir_bytes timeout;


 __uint8_t padding[24];



} mess_lc_vfs_select;
typedef int _ASSERT_mess_lc_vfs_select[ sizeof(mess_lc_vfs_select) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 vir_bytes buf;
 size_t len;
 int flags;
 vir_bytes addr;
 unsigned int addr_len;


 __uint8_t padding[20];



} mess_lc_vfs_sendrecv;
typedef int _ASSERT_mess_lc_vfs_sendrecv[ sizeof(mess_lc_vfs_sendrecv) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 int how;

 __uint8_t padding[48];
} mess_lc_vfs_shutdown;
typedef int _ASSERT_mess_lc_vfs_shutdown[ sizeof(mess_lc_vfs_shutdown) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 vir_bytes addr;
 unsigned int addr_len;

 __uint8_t padding[44];
} mess_lc_vfs_sockaddr;
typedef int _ASSERT_mess_lc_vfs_sockaddr[ sizeof(mess_lc_vfs_sockaddr) <= 64 ? 1 : -1];

typedef struct {
 int domain;
 int type;
 int protocol;

 __uint8_t padding[44];
} mess_lc_vfs_socket;
typedef int _ASSERT_mess_lc_vfs_socket[ sizeof(mess_lc_vfs_socket) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 vir_bytes msgbuf;
 int flags;

 __uint8_t padding[44];
} mess_lc_vfs_sockmsg;
typedef int _ASSERT_mess_lc_vfs_sockmsg[ sizeof(mess_lc_vfs_sockmsg) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 int level;
 int name;
 vir_bytes buf;
 unsigned int len;

 __uint8_t padding[36];
} mess_lc_vfs_sockopt;
typedef int _ASSERT_mess_lc_vfs_sockopt[ sizeof(mess_lc_vfs_sockopt) <= 64 ? 1 : -1];

typedef struct {
 size_t len;
 vir_bytes name;
 vir_bytes buf;


 __uint8_t padding[40];



} mess_lc_vfs_stat;
typedef int _ASSERT_mess_lc_vfs_stat[ sizeof(mess_lc_vfs_stat) <= 64 ? 1 : -1];

typedef struct {
 int fd;
 int flags;
 size_t len;
 vir_bytes name;
 vir_bytes buf;


 __uint8_t padding[32];



} mess_lc_vfs_statvfs1;
typedef int _ASSERT_mess_lc_vfs_statvfs1[ sizeof(mess_lc_vfs_statvfs1) <= 64 ? 1 : -1];

typedef struct {
 __off_t offset;

 int fd;
 vir_bytes name;
 size_t len;


 __uint8_t padding[32];



} mess_lc_vfs_truncate;
typedef int _ASSERT_mess_lc_vfs_truncate[ sizeof(mess_lc_vfs_truncate) <= 64 ? 1 : -1];

typedef struct {
 __mode_t mask;

 __uint8_t padding[52];
} mess_lc_vfs_umask;
typedef int _ASSERT_mess_lc_vfs_umask[ sizeof(mess_lc_vfs_umask) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes name;
 size_t namelen;
 vir_bytes label;
 size_t labellen;


 __uint8_t padding[32];



} mess_lc_vfs_umount;
typedef int _ASSERT_mess_lc_vfs_umount[ sizeof(mess_lc_vfs_umount) <= 64 ? 1 : -1];

typedef struct {
 void *addr;
 __uint8_t padding[52];
} mess_lc_vm_brk;
typedef int _ASSERT_mess_lc_vm_brk[ sizeof(mess_lc_vm_brk) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 void *addr;
 void *ret_addr;

 __uint8_t padding[40];



} mess_lc_vm_getphys;
typedef int _ASSERT_mess_lc_vm_getphys[ sizeof(mess_lc_vm_getphys) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t forwhom;
 void *addr;
 __uint8_t padding[48];
} mess_lc_vm_shm_unmap;
typedef int _ASSERT_mess_lc_vm_shm_unmap[ sizeof(mess_lc_vm_shm_unmap) <= 64 ? 1 : -1];

typedef struct {
 int status;
 __uint32_t id;

 __uint8_t padding[48];
} mess_lchardriver_vfs_reply;
typedef int _ASSERT_mess_lchardriver_vfs_reply[ sizeof(mess_lchardriver_vfs_reply) <= 64 ? 1 : -1];

typedef struct {
 int status;
 __int32_t minor;

 __uint8_t padding[48];
} mess_lchardriver_vfs_sel1;
typedef int _ASSERT_mess_lchardriver_vfs_sel1[ sizeof(mess_lchardriver_vfs_sel1) <= 64 ? 1 : -1];

typedef struct {
 int status;
 __int32_t minor;

 __uint8_t padding[48];
} mess_lchardriver_vfs_sel2;
typedef int _ASSERT_mess_lchardriver_vfs_sel2[ sizeof(mess_lchardriver_vfs_sel2) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes ptr;

 __uint8_t padding[48];
} mess_lexec_pm_exec_new;
typedef int _ASSERT_mess_lexec_pm_exec_new[ sizeof(mess_lexec_pm_exec_new) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t grant;

 __uint8_t padding[52];
} mess_li2cdriver_i2c_busc_i2c_exec;
typedef int _ASSERT_mess_li2cdriver_i2c_busc_i2c_exec[ sizeof(mess_li2cdriver_i2c_busc_i2c_exec) <= 64 ? 1 : -1];

typedef struct {
 __uint16_t addr;




 __uint8_t padding[54];
} mess_li2cdriver_i2c_busc_i2c_reserve;
typedef int _ASSERT_mess_li2cdriver_i2c_busc_i2c_reserve[ sizeof(mess_li2cdriver_i2c_busc_i2c_reserve) <= 64 ? 1 : -1];

typedef struct {
 int id;
 int page;
 int code;
 int value;
 int flags;

 __uint8_t padding[36];
} mess_linputdriver_input_event;
typedef int _ASSERT_mess_linputdriver_input_event[ sizeof(mess_linputdriver_input_event) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 __int32_t sock_id;
 int status;
 unsigned int len;

 __uint8_t padding[40];
} mess_lsockdriver_vfs_accept_reply;
typedef int _ASSERT_mess_lsockdriver_vfs_accept_reply[ sizeof(mess_lsockdriver_vfs_accept_reply) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 int status;
 unsigned int ctl_len;
 unsigned int addr_len;
 int flags;

 __uint8_t padding[36];
} mess_lsockdriver_vfs_recv_reply;
typedef int _ASSERT_mess_lsockdriver_vfs_recv_reply[ sizeof(mess_lsockdriver_vfs_recv_reply) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 int status;

 __uint8_t padding[48];
} mess_lsockdriver_vfs_reply;
typedef int _ASSERT_mess_lsockdriver_vfs_reply[ sizeof(mess_lsockdriver_vfs_reply) <= 64 ? 1 : -1];

typedef struct {
 __int32_t sock_id;
 int status;

 __uint8_t padding[48];
} mess_lsockdriver_vfs_select_reply;
typedef int _ASSERT_mess_lsockdriver_vfs_select_reply[ sizeof(mess_lsockdriver_vfs_select_reply) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 __int32_t sock_id;
 __int32_t sock_id2;

 __uint8_t padding[44];
} mess_lsockdriver_vfs_socket_reply;
typedef int _ASSERT_mess_lsockdriver_vfs_socket_reply[ sizeof(mess_lsockdriver_vfs_socket_reply) <= 64 ? 1 : -1];

typedef struct {
        cp_grant_id_t gid;
 size_t size;
 int subtype;

        __uint8_t padding[44];
} mess_lsys_fi_ctl;
typedef int _ASSERT_mess_lsys_fi_ctl[ sizeof(mess_lsys_fi_ctl) <= 64 ? 1 : -1];

typedef struct {
        int status;

        __uint8_t padding[52];
} mess_lsys_fi_reply;
typedef int _ASSERT_mess_lsys_fi_reply[ sizeof(mess_lsys_fi_reply) <= 64 ? 1 : -1];

typedef struct {
 int what;
 vir_bytes where;
 size_t size;


 __uint8_t padding[40];



} mess_lsys_getsysinfo;
typedef int _ASSERT_mess_lsys_getsysinfo[ sizeof(mess_lsys_getsysinfo) <= 64 ? 1 : -1];

typedef struct {
 size_t size;
 phys_bytes addr;
 vir_bytes buf;


 __uint8_t padding[40];



} mess_lsys_krn_readbios;
typedef int _ASSERT_mess_lsys_krn_readbios[ sizeof(mess_lsys_krn_readbios) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t from_to;
 cp_grant_id_t gid;
 size_t offset;
 void *address;
 size_t bytes;

 __uint8_t padding[32];



} mess_lsys_kern_safecopy;
typedef int _ASSERT_mess_lsys_kern_safecopy[ sizeof(mess_lsys_kern_safecopy) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t flags;
 endpoint_t endpoint;
 int priority;
 int quantum;
 int cpu;

 __uint8_t padding[36];
} mess_lsys_krn_schedctl;
typedef int _ASSERT_mess_lsys_krn_schedctl[ sizeof(mess_lsys_krn_schedctl) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpoint;
 int quantum;
 int priority;
 int cpu;
 int niced;

 __uint8_t padding[36];
} mess_lsys_krn_schedule;
typedef int _ASSERT_mess_lsys_krn_schedule[ sizeof(mess_lsys_krn_schedule) <= 64 ? 1 : -1];

typedef struct {
 int how;

 __uint8_t padding[52];
} mess_lsys_krn_sys_abort;
typedef int _ASSERT_mess_lsys_krn_sys_abort[ sizeof(mess_lsys_krn_sys_abort) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;

 __uint8_t padding[52];
} mess_lsys_krn_sys_clear;
typedef int _ASSERT_mess_lsys_krn_sys_clear[ sizeof(mess_lsys_krn_sys_clear) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t src_endpt;
 vir_bytes src_addr;
 endpoint_t dst_endpt;
 vir_bytes dst_addr;
 phys_bytes nr_bytes;
 int flags;


 __uint8_t padding[20];



} mess_lsys_krn_sys_copy;
typedef int _ASSERT_mess_lsys_krn_sys_copy[ sizeof(mess_lsys_krn_sys_copy) <= 64 ? 1 : -1];

typedef struct {
 int request;
 int port;
 __uint32_t value;

 __uint8_t padding[44];
} mess_lsys_krn_sys_devio;
typedef int _ASSERT_mess_lsys_krn_sys_devio[ sizeof(mess_lsys_krn_sys_devio) <= 64 ? 1 : -1];

typedef struct {
 int code;
 vir_bytes buf;
 int len;
 endpoint_t endpt;

 __uint8_t padding[40];
} mess_lsys_krn_sys_diagctl;
typedef int _ASSERT_mess_lsys_krn_sys_diagctl[ sizeof(mess_lsys_krn_sys_diagctl) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes ip;
 vir_bytes stack;
 vir_bytes name;
 vir_bytes ps_str;


 __uint8_t padding[24];



} mess_lsys_krn_sys_exec;
typedef int _ASSERT_mess_lsys_krn_sys_exec[ sizeof(mess_lsys_krn_sys_exec) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 endpoint_t slot;
 __uint32_t flags;

 __uint8_t padding[44];
} mess_lsys_krn_sys_fork;
typedef int _ASSERT_mess_lsys_krn_sys_fork[ sizeof(mess_lsys_krn_sys_fork) <= 64 ? 1 : -1];

typedef struct {
 int request;
 endpoint_t endpt;
 vir_bytes val_ptr;
 int val_len;
 vir_bytes val_ptr2;
 int val_len2_e;


 __uint8_t padding[28];



} mess_lsys_krn_sys_getinfo;
typedef int _ASSERT_mess_lsys_krn_sys_getinfo[ sizeof(mess_lsys_krn_sys_getinfo) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes ctx_ptr;

 __uint8_t padding[48];
} mess_lsys_krn_sys_getmcontext;
typedef int _ASSERT_mess_lsys_krn_sys_getmcontext[ sizeof(mess_lsys_krn_sys_getmcontext) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;

 __uint8_t padding[52];
} mess_lsys_krn_sys_iopenable;
typedef int _ASSERT_mess_lsys_krn_sys_iopenable[ sizeof(mess_lsys_krn_sys_iopenable) <= 64 ? 1 : -1];

typedef struct {
 int request;
 int vector;
 int policy;
 int hook_id;

 __uint8_t padding[40];
} mess_lsys_krn_sys_irqctl;
typedef int _ASSERT_mess_lsys_krn_sys_irqctl[ sizeof(mess_lsys_krn_sys_irqctl) <= 64 ? 1 : -1];

typedef struct {
 phys_bytes base;
 phys_bytes count;
 unsigned long pattern;
 endpoint_t process;


 __uint8_t padding[36];



} mess_lsys_krn_sys_memset;
typedef int _ASSERT_mess_lsys_krn_sys_memset[ sizeof(mess_lsys_krn_sys_memset) <= 64 ? 1 : -1];

typedef struct {
 int request;
 endpoint_t endpt;
 vir_bytes arg_ptr;
 phys_bytes phys_start;
 phys_bytes phys_len;


 __uint8_t padding[32];



} mess_lsys_krn_sys_privctl;
typedef int _ASSERT_mess_lsys_krn_sys_privctl[ sizeof(mess_lsys_krn_sys_privctl) <= 64 ? 1 : -1];


typedef struct {
 int request;
 int port;
 endpoint_t vec_endpt;
 phys_bytes vec_addr;
 vir_bytes vec_size;
 vir_bytes offset;

 __uint8_t padding[24];
} mess_lsys_krn_sys_sdevio;
# 1424 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef int _ASSERT_mess_lsys_krn_sys_sdevio[ sizeof(mess_lsys_krn_sys_sdevio) <= 64 ? 1 : -1];

typedef struct {
 clock_t exp_time;
 clock_t time_left;
 clock_t uptime;
 int abs_time;

 __uint8_t padding[40];
} mess_lsys_krn_sys_setalarm;
typedef int _ASSERT_mess_lsys_krn_sys_setalarm[ sizeof(mess_lsys_krn_sys_setalarm) <= 64 ? 1 : -1];

typedef struct {
 vir_bytes addr;
 int size;

 __uint8_t padding[48];
} mess_lsys_krn_sys_setgrant;
typedef int _ASSERT_mess_lsys_krn_sys_setgrant[ sizeof(mess_lsys_krn_sys_setgrant) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes ctx_ptr;

 __uint8_t padding[48];
} mess_lsys_krn_sys_setmcontext;
typedef int _ASSERT_mess_lsys_krn_sys_setmcontext[ sizeof(mess_lsys_krn_sys_setmcontext) <= 64 ? 1 : -1];

typedef struct {
 time_t sec;
 long int nsec;
 int now;
 clockid_t clock_id;

 __uint8_t padding[36];
} mess_lsys_krn_sys_settime;
typedef int _ASSERT_mess_lsys_krn_sys_settime[ sizeof(mess_lsys_krn_sys_settime) <= 64 ? 1 : -1];

typedef struct {
 int action;
 int freq;
 int intr_type;
 endpoint_t endpt;
 vir_bytes ctl_ptr;
 vir_bytes mem_ptr;
 size_t mem_size;


 __uint8_t padding[24];



} mess_lsys_krn_sys_sprof;
typedef int _ASSERT_mess_lsys_krn_sys_sprof[ sizeof(mess_lsys_krn_sys_sprof) <= 64 ? 1 : -1];

typedef struct {
 int request;
 void *address;
 int length;

 __uint8_t padding[44];
} mess_lsys_krn_sys_statectl;
typedef int _ASSERT_mess_lsys_krn_sys_statectl[ sizeof(mess_lsys_krn_sys_statectl) <= 64 ? 1 : -1];

typedef struct {
 time_t boot_time;

 __uint8_t padding[48];
} mess_lsys_krn_sys_stime;
typedef int _ASSERT_mess_lsys_krn_sys_stime[ sizeof(mess_lsys_krn_sys_stime) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;

 __uint8_t padding[52];
} mess_lsys_krn_sys_times;
typedef int _ASSERT_mess_lsys_krn_sys_times[ sizeof(mess_lsys_krn_sys_times) <= 64 ? 1 : -1];

typedef struct {
 int request;
 endpoint_t endpt;
 vir_bytes address;
 long int data;

 __uint8_t padding[40];
} mess_lsys_krn_sys_trace;
typedef int _ASSERT_mess_lsys_krn_sys_trace[ sizeof(mess_lsys_krn_sys_trace) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t src_endpt;
 int segment;
 vir_bytes src_addr;
 endpoint_t dst_endpt;
 int nr_bytes;

 __uint8_t padding[36];
} mess_lsys_krn_sys_umap;
typedef int _ASSERT_mess_lsys_krn_sys_umap[ sizeof(mess_lsys_krn_sys_umap) <= 64 ? 1 : -1];


typedef struct {
 int request;
 int vec_size;
 vir_bytes vec_addr;

 __uint8_t padding[44];
} mess_lsys_krn_sys_vdevio;
typedef int _ASSERT_mess_lsys_krn_sys_vdevio[ sizeof(mess_lsys_krn_sys_vdevio) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes vaddr;
 int vcount;
 vir_bytes paddr;
 int pmax;
 int access;
 size_t offset;


 __uint8_t padding[16];



} mess_lsys_krn_sys_vumap;
typedef int _ASSERT_mess_lsys_krn_sys_vumap[ sizeof(mess_lsys_krn_sys_vumap) <= 64 ? 1 : -1];

typedef struct {
 void *vec_addr;
 int vec_size;
 __uint8_t padding[48];
} mess_lsys_kern_vsafecopy;
typedef int _ASSERT_mess_lsys_kern_vsafecopy[ sizeof(mess_lsys_kern_vsafecopy) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t root_id;
 __uint32_t flags;
 unsigned int csize;
 unsigned int clen;
 unsigned int miblen;
 int mib[8];
 __uint8_t padding[4];
} mess_lsys_mib_register;
typedef int _ASSERT_mess_lsys_mib_register[ sizeof(mess_lsys_mib_register) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t req_id;
 ssize_t status;
 __uint8_t padding[48];
} mess_lsys_mib_reply;
typedef int _ASSERT_mess_lsys_mib_reply[ sizeof(mess_lsys_mib_reply) <= 64 ? 1 : -1];

typedef struct {
 int devind;
 int port;

 __uint8_t padding[48];
} mess_lsys_pci_busc_get_bar;
typedef int _ASSERT_mess_lsys_pci_busc_get_bar[ sizeof(mess_lsys_pci_busc_get_bar) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes groups;
 int ngroups;

 __uint8_t padding[44];
} mess_lsys_pm_getepinfo;
typedef int _ASSERT_mess_lsys_pm_getepinfo[ sizeof(mess_lsys_pm_getepinfo) <= 64 ? 1 : -1];

typedef struct {
 __pid_t pid;

 __uint8_t padding[52];
} mess_lsys_pm_getprocnr;
typedef int _ASSERT_mess_lsys_pm_getprocnr[ sizeof(mess_lsys_pm_getprocnr) <= 64 ? 1 : -1];

typedef struct {
 unsigned int mask;

 __uint8_t padding[52];
} mess_lsys_pm_proceventmask;
typedef int _ASSERT_mess_lsys_pm_proceventmask[ sizeof(mess_lsys_pm_proceventmask) <= 64 ? 1 : -1];

typedef struct {
 __uid_t uid;
 __gid_t gid;

 __uint8_t padding[48];
} mess_lsys_pm_srv_fork;
typedef int _ASSERT_mess_lsys_pm_srv_fork[ sizeof(mess_lsys_pm_srv_fork) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpoint;
 endpoint_t parent;
 int maxprio;
 int quantum;

 __uint8_t padding[40];
} mess_lsys_sched_scheduling_start;
typedef int _ASSERT_mess_lsys_sched_scheduling_start[ sizeof(mess_lsys_sched_scheduling_start) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpoint;

 __uint8_t padding[52];
} mess_lsys_sched_scheduling_stop;
typedef int _ASSERT_mess_lsys_sched_scheduling_stop[ sizeof(mess_lsys_sched_scheduling_stop) <= 64 ? 1 : -1];

typedef struct {
 int request;
 int fkeys;
 int sfkeys;

 __uint8_t padding[44];
} mess_lsys_tty_fkey_ctl;
typedef int _ASSERT_mess_lsys_tty_fkey_ctl[ sizeof(mess_lsys_tty_fkey_ctl) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 int fd;
 int what;

 __uint8_t padding[44];
} mess_lsys_vfs_copyfd;
typedef int _ASSERT_mess_lsys_vfs_copyfd[ sizeof(mess_lsys_vfs_copyfd) <= 64 ? 1 : -1];


typedef struct {
 __devmajor_t major;
 __uint32_t labellen;
 vir_bytes label;
 int ndomains;
 int domains[8];

 __uint8_t padding[8];
} mess_lsys_vfs_mapdriver;
# 1670 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef int _ASSERT_mess_lsys_vfs_mapdriver[ sizeof(mess_lsys_vfs_mapdriver) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 cp_grant_id_t grant;
 size_t count;
 int what;

 __uint8_t padding[40];
} mess_lsys_vfs_socketpath;
typedef int _ASSERT_mess_lsys_vfs_socketpath[ sizeof(mess_lsys_vfs_socketpath) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 void *addr;
 int retc;
 __uint8_t padding[44];
} mess_lsys_vm_getref;
typedef int _ASSERT_mess_lsys_vm_getref[ sizeof(mess_lsys_vm_getref) <= 64 ? 1 : -1];

typedef struct {
 int what;
 endpoint_t ep;
 int count;
 void *ptr;
 vir_bytes next;

 __uint8_t padding[32];



} mess_lsys_vm_info;
typedef int _ASSERT_mess_lsys_vm_info[ sizeof(mess_lsys_vm_info) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t ep;
 phys_bytes phaddr;
 size_t len;
 void *reply;

 __uint8_t padding[32];



} mess_lsys_vm_map_phys;
typedef int _ASSERT_mess_lsys_vm_map_phys[ sizeof(mess_lsys_vm_map_phys) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 vir_bytes addr;
 int children;

 __uint8_t padding[44];
} mess_lsys_vm_rusage;
typedef int _ASSERT_mess_lsys_vm_rusage[ sizeof(mess_lsys_vm_rusage) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t ep;
 void *vaddr;
 __uint8_t padding[48];
} mess_lsys_vm_unmap_phys;
typedef int _ASSERT_mess_lsys_vm_unmap_phys[ sizeof(mess_lsys_vm_unmap_phys) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t src;
 endpoint_t dst;
 int flags;
 __uint8_t padding[44];
} mess_lsys_vm_update;
typedef int _ASSERT_mess_lsys_vm_update[ sizeof(mess_lsys_vm_update) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t destination;
 endpoint_t source;
 void *dest_addr;
 void *src_addr;
 size_t size;
 void *ret_addr;

 __uint8_t padding[24];



} mess_lsys_vm_vmremap;
typedef int _ASSERT_mess_lsys_vm_vmremap[ sizeof(mess_lsys_vm_vmremap) <= 64 ? 1 : -1];

typedef struct {
 size_t oldlen;
 __uint8_t padding[52];
} mess_mib_lc_sysctl;
typedef int _ASSERT_mess_mib_lc_sysctl[ sizeof(mess_mib_lc_sysctl) <= 64 ? 1 : -1];


typedef struct {
 __uint32_t req_id;
 __uint32_t root_id;
 cp_grant_id_t name_grant;
 unsigned int name_len;
 cp_grant_id_t oldp_grant;
 __uint32_t oldp_len;
 cp_grant_id_t newp_grant;
 __uint32_t newp_len;
 endpoint_t user_endpt;
 __uint32_t flags;
 __uint32_t root_ver;
 __uint32_t tree_ver;
 __uint8_t padding[16];
} mess_mib_lsys_call;
# 1795 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef int _ASSERT_mess_mib_lsys_call[ sizeof(mess_mib_lsys_call) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t req_id;
 __uint32_t root_id;
 cp_grant_id_t name_grant;
 size_t name_size;
 cp_grant_id_t desc_grant;
 size_t desc_size;

 __uint8_t padding[24];



} mess_mib_lsys_info;
typedef int _ASSERT_mess_mib_lsys_info[ sizeof(mess_mib_lsys_info) <= 64 ? 1 : -1];

typedef struct {
 __off_t offset;
 void *addr;
 size_t len;
 int prot;
 int flags;
 int fd;
 endpoint_t forwhom;
 void *retaddr;

 __uint32_t padding[4];



} mess_mmap;
typedef int _ASSERT_mess_mmap[ sizeof(mess_mmap) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t id;

 __uint8_t padding[52];
} mess_ndev_netdriver_init;
typedef int _ASSERT_mess_ndev_netdriver_init[ sizeof(mess_ndev_netdriver_init) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t id;
 __uint32_t set;
 __uint32_t mode;
 cp_grant_id_t mcast_grant;
 unsigned int mcast_count;
 __uint32_t caps;
 __uint32_t flags;
 __uint32_t media;
 __uint8_t hwaddr[6];

 __uint8_t padding[18];
} mess_ndev_netdriver_conf;
typedef int _ASSERT_mess_ndev_netdriver_conf[ sizeof(mess_ndev_netdriver_conf) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t id;
 __uint32_t count;
 cp_grant_id_t grant[8];
 __uint16_t len[8];
} mess_ndev_netdriver_transfer;
typedef int _ASSERT_mess_ndev_netdriver_transfer[ sizeof(mess_ndev_netdriver_transfer) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t id;

 __uint8_t padding[52];
} mess_ndev_netdriver_status_reply;
typedef int _ASSERT_mess_ndev_netdriver_status_reply[ sizeof(mess_ndev_netdriver_status_reply) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t id;
 __uint32_t link;
 __uint32_t media;
 __uint32_t caps;
 char name[16];
 __uint8_t hwaddr[6];
 __uint8_t hwaddr_len;
 __uint8_t max_send;
 __uint8_t max_recv;

 __uint8_t padding[15];
} mess_netdriver_ndev_init_reply;
typedef int _ASSERT_mess_netdriver_ndev_init_reply[ sizeof(mess_netdriver_ndev_init_reply) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t id;
 __int32_t result;

 __uint8_t padding[48];
} mess_netdriver_ndev_reply;
typedef int _ASSERT_mess_netdriver_ndev_reply[ sizeof(mess_netdriver_ndev_reply) <= 64 ? 1 : -1];

typedef struct {
 __uint32_t id;
 __uint32_t link;
 __uint32_t media;
 __uint32_t oerror;
 __uint32_t coll;
 __uint32_t ierror;
 __uint32_t iqdrop;

 __uint8_t padding[28];
} mess_netdriver_ndev_status;
typedef int _ASSERT_mess_netdriver_ndev_status[ sizeof(mess_netdriver_ndev_status) <= 64 ? 1 : -1];

typedef struct {
 int mode;

 __uint8_t padding[52];
} mess_net_netdrv_dl_conf;
typedef int _ASSERT_mess_net_netdrv_dl_conf[ sizeof(mess_net_netdrv_dl_conf) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t grant;

 __uint8_t padding[52];
} mess_net_netdrv_dl_getstat_s;
typedef int _ASSERT_mess_net_netdrv_dl_getstat_s[ sizeof(mess_net_netdrv_dl_getstat_s) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t grant;
 int count;

 __uint8_t padding[48];
} mess_net_netdrv_dl_readv_s;
typedef int _ASSERT_mess_net_netdrv_dl_readv_s[ sizeof(mess_net_netdrv_dl_readv_s) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t grant;
 int count;

 __uint8_t padding[48];
} mess_net_netdrv_dl_writev_s;
typedef int _ASSERT_mess_net_netdrv_dl_writev_s[ sizeof(mess_net_netdrv_dl_writev_s) <= 64 ? 1 : -1];

typedef struct {
 int stat;
 __uint8_t hw_addr[6];

 __uint8_t padding[46];
} mess_netdrv_net_dl_conf;
typedef int _ASSERT_mess_netdrv_net_dl_conf[ sizeof(mess_netdrv_net_dl_conf) <= 64 ? 1 : -1];

typedef struct {
 int count;
 __uint32_t flags;

 __uint8_t padding[48];
} mess_netdrv_net_dl_task;
typedef int _ASSERT_mess_netdrv_net_dl_task[ sizeof(mess_netdrv_net_dl_task) <= 64 ? 1 : -1];

typedef struct {
 __uint64_t timestamp;
 __uint64_t interrupts;
 sigset_t sigset;
 __uint8_t padding[24];
} mess_notify;
typedef int _ASSERT_mess_notify[ sizeof(mess_notify) <= 64 ? 1 : -1];

typedef struct {
 int base;
 size_t size;
 __uint32_t flags;

 __uint8_t padding[44];
} mess_pci_lsys_busc_get_bar;
typedef int _ASSERT_mess_pci_lsys_busc_get_bar[ sizeof(mess_pci_lsys_busc_get_bar) <= 64 ? 1 : -1];

typedef struct {
 __uid_t egid;

 __uint8_t padding[52];
} mess_pm_lc_getgid;
typedef int _ASSERT_mess_pm_lc_getgid[ sizeof(mess_pm_lc_getgid) <= 64 ? 1 : -1];

typedef struct {
 __pid_t parent_pid;

 __uint8_t padding[52];
} mess_pm_lc_getpid;
typedef int _ASSERT_mess_pm_lc_getpid[ sizeof(mess_pm_lc_getpid) <= 64 ? 1 : -1];

typedef struct {
 __uid_t euid;

 __uint8_t padding[52];
} mess_pm_lc_getuid;
typedef int _ASSERT_mess_pm_lc_getuid[ sizeof(mess_pm_lc_getuid) <= 64 ? 1 : -1];

typedef struct {
 long data;

 __uint8_t padding[52];
} mess_pm_lc_ptrace;
typedef int _ASSERT_mess_pm_lc_ptrace[ sizeof(mess_pm_lc_ptrace) <= 64 ? 1 : -1];

typedef struct {
 sigset_t set;

 __uint8_t padding[40];
} mess_pm_lc_sigset;
typedef int _ASSERT_mess_pm_lc_sigset[ sizeof(mess_pm_lc_sigset) <= 64 ? 1 : -1];

typedef struct {
 time_t sec;

 long nsec;

 __uint8_t padding[44];
} mess_pm_lc_time;
typedef int _ASSERT_mess_pm_lc_time[ sizeof(mess_pm_lc_time) <= 64 ? 1 : -1];

typedef struct {
 int status;

 __uint8_t padding[52];
} mess_pm_lc_wait4;
typedef int _ASSERT_mess_pm_lc_wait4[ sizeof(mess_pm_lc_wait4) <= 64 ? 1 : -1];

typedef struct {
 int suid;

 __uint8_t padding[52];
} mess_pm_lexec_exec_new;
typedef int _ASSERT_mess_pm_lexec_exec_new[ sizeof(mess_pm_lexec_exec_new) <= 64 ? 1 : -1];

typedef struct {
 __uid_t uid;
 __uid_t euid;
 __gid_t gid;
 __gid_t egid;
 int ngroups;

 __uint8_t padding[36];
} mess_pm_lsys_getepinfo;
typedef int _ASSERT_mess_pm_lsys_getepinfo[ sizeof(mess_pm_lsys_getepinfo) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;

 __uint8_t padding[52];
} mess_pm_lsys_getprocnr;
typedef int _ASSERT_mess_pm_lsys_getprocnr[ sizeof(mess_pm_lsys_getprocnr) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 unsigned int event;

 __uint8_t padding[48];
} mess_pm_lsys_proc_event;
typedef int _ASSERT_mess_pm_lsys_proc_event[ sizeof(mess_pm_lsys_proc_event) <= 64 ? 1 : -1];

typedef struct {
 int num;

 __uint8_t padding[52];
} mess_pm_lsys_sigs_signal;
typedef int _ASSERT_mess_pm_lsys_sigs_signal[ sizeof(mess_pm_lsys_sigs_signal) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpoint;
 __uint32_t maxprio;

 __uint8_t padding[48];
} mess_pm_sched_scheduling_set_nice;
typedef int _ASSERT_mess_pm_sched_scheduling_set_nice[ sizeof(mess_pm_sched_scheduling_set_nice) <= 64 ? 1 : -1];

typedef struct {
 dev_t dev;
 __mode_t mode;
 __uid_t uid;
 __gid_t gid;
 __uint32_t index;

 __uint8_t padding[32];
} mess_pty_ptyfs_req;
typedef int _ASSERT_mess_pty_ptyfs_req[ sizeof(mess_pty_ptyfs_req) <= 64 ? 1 : -1];

typedef struct {
 char name[20];

 __uint8_t padding[36];
} mess_ptyfs_pty_name;
typedef int _ASSERT_mess_ptyfs_pty_name[ sizeof(mess_ptyfs_pty_name) <= 64 ? 1 : -1];

typedef struct {
 int status;

 __uint8_t padding[52];
} mess_readclock_lc_rtcdev;
typedef int _ASSERT_mess_readclock_lc_rtcdev[ sizeof(mess_readclock_lc_rtcdev) <= 64 ? 1 : -1];

typedef struct {
 int result;
 int type;
 cp_grant_id_t rproctab_gid;
 endpoint_t old_endpoint;
 int restarts;
 int flags;
 vir_bytes buff_addr;
 size_t buff_len;
 int prepare_state;
 __uint8_t padding[20];
} mess_rs_init;
typedef int _ASSERT_mess_rs_init[ sizeof(mess_rs_init) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t endpt;
 int result;
 vir_bytes pc;
 vir_bytes ps_str;

 __uint8_t padding[40];
} mess_rs_pm_exec_restart;
typedef int _ASSERT_mess_rs_pm_exec_restart[ sizeof(mess_rs_pm_exec_restart) <= 64 ? 1 : -1];

typedef struct {
 __pid_t pid;
 int nr;

 __uint8_t padding[48];
} mess_rs_pm_srv_kill;
typedef int _ASSERT_mess_rs_pm_srv_kill[ sizeof(mess_rs_pm_srv_kill) <= 64 ? 1 : -1];

typedef struct {
 int len;
 int name_len;
 endpoint_t endpoint;
 void *addr;
 const char *name;
 int subtype;

 __uint8_t padding[28];



} mess_rs_req;
typedef int _ASSERT_mess_rs_req[ sizeof(mess_rs_req) <= 64 ? 1 : -1];

typedef struct {
 int result;
 int state;
 int prepare_maxtime;
 int flags;
 __gid_t state_data_gid;
 __uint8_t padding[36];
} mess_rs_update;
typedef int _ASSERT_mess_rs_update[ sizeof(mess_rs_update) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t scheduler;

 __uint8_t padding[52];
} mess_sched_lsys_scheduling_start;
typedef int _ASSERT_mess_sched_lsys_scheduling_start[ sizeof(mess_sched_lsys_scheduling_start) <= 64 ? 1 : -1];


typedef struct {
 sigset_t map;
 endpoint_t endpt;
 int sig;
 void *sigctx;
 __uint8_t padding[28];
} mess_sigcalls;
typedef int _ASSERT_mess_sigcalls[ sizeof(mess_sigcalls) <= 64 ? 1 : -1];

typedef struct {
 int fkeys;
 int sfkeys;

 __uint8_t padding[48];
} mess_tty_lsys_fkey_ctl;
typedef int _ASSERT_mess_tty_lsys_fkey_ctl[ sizeof(mess_tty_lsys_fkey_ctl) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;
 __off_t seek_pos;

 cp_grant_id_t grant;
 size_t nbytes;

 __uint8_t data[32];
} mess_vfs_fs_breadwrite;
typedef int _ASSERT_mess_vfs_fs_breadwrite[ sizeof(mess_vfs_fs_breadwrite) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 __mode_t mode;

 __uint8_t data[44];
} mess_vfs_fs_chmod;
typedef int _ASSERT_mess_vfs_fs_chmod[ sizeof(mess_vfs_fs_chmod) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 __uid_t uid;
 __gid_t gid;

 __uint8_t data[40];
} mess_vfs_fs_chown;
typedef int _ASSERT_mess_vfs_fs_chown[ sizeof(mess_vfs_fs_chown) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 __mode_t mode;
 __uid_t uid;
 __gid_t gid;
 cp_grant_id_t grant;
 size_t path_len;

 __uint8_t data[28];
} mess_vfs_fs_create;
typedef int _ASSERT_mess_vfs_fs_create[ sizeof(mess_vfs_fs_create) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;

 __uint8_t data[48];
} mess_vfs_fs_flush;
typedef int _ASSERT_mess_vfs_fs_flush[ sizeof(mess_vfs_fs_flush) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;
 __off_t trc_start;
 __off_t trc_end;

 __uint8_t data[32];
} mess_vfs_fs_ftrunc;
typedef int _ASSERT_mess_vfs_fs_ftrunc[ sizeof(mess_vfs_fs_ftrunc) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;
 __off_t seek_pos;

 cp_grant_id_t grant;
 size_t mem_size;

 __uint8_t data[32];
} mess_vfs_fs_getdents;
typedef int _ASSERT_mess_vfs_fs_getdents[ sizeof(mess_vfs_fs_getdents) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 __uint8_t data[48];
} mess_vfs_fs_inhibread;
typedef int _ASSERT_mess_vfs_fs_inhibread[ sizeof(mess_vfs_fs_inhibread) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;
 ino_t dir_ino;

 cp_grant_id_t grant;
 size_t path_len;

 __uint8_t data[32];
} mess_vfs_fs_link;
typedef int _ASSERT_mess_vfs_fs_link[ sizeof(mess_vfs_fs_link) <= 64 ? 1 : -1];


typedef struct {
 ino_t dir_ino;
 ino_t root_ino;
 __uint32_t flags;
 __uint32_t path_len;
 __uint32_t path_size;
 __uint32_t ucred_size;
 cp_grant_id_t grant_path;
 cp_grant_id_t grant_ucred;
 __uid_t uid;
 __gid_t gid;

 __uint8_t data[8];
} mess_vfs_fs_lookup;
# 2290 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef int _ASSERT_mess_vfs_fs_lookup[ sizeof(mess_vfs_fs_lookup) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 __mode_t mode;
 __uid_t uid;
 __gid_t gid;
 cp_grant_id_t grant;
 size_t path_len;

 __uint8_t data[28];
} mess_vfs_fs_mkdir;
typedef int _ASSERT_mess_vfs_fs_mkdir[ sizeof(mess_vfs_fs_mkdir) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;
 ino_t inode;

 __mode_t mode;
 __uid_t uid;
 __gid_t gid;
 cp_grant_id_t grant;
 size_t path_len;

 __uint8_t data[20];
} mess_vfs_fs_mknod;
typedef int _ASSERT_mess_vfs_fs_mknod[ sizeof(mess_vfs_fs_mknod) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 __uint8_t data[48];
} mess_vfs_fs_mountpoint;
typedef int _ASSERT_mess_vfs_fs_mountpoint[ sizeof(mess_vfs_fs_mountpoint) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;

 cp_grant_id_t grant;
 size_t path_len;

 __uint8_t data[40];
} mess_vfs_fs_new_driver;
typedef int _ASSERT_mess_vfs_fs_new_driver[ sizeof(mess_vfs_fs_new_driver) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;

 __mode_t mode;
 __uid_t uid;
 __gid_t gid;

 __uint8_t data[36];
} mess_vfs_fs_newnode;
typedef int _ASSERT_mess_vfs_fs_newnode[ sizeof(mess_vfs_fs_newnode) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;
 unsigned int count;

 __uint8_t data[44];
} mess_vfs_fs_putnode;
typedef int _ASSERT_mess_vfs_fs_putnode[ sizeof(mess_vfs_fs_putnode) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 cp_grant_id_t grant;
 size_t mem_size;

 __uint8_t data[40];
} mess_vfs_fs_rdlink;
typedef int _ASSERT_mess_vfs_fs_rdlink[ sizeof(mess_vfs_fs_rdlink) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;

 __uint32_t flags;
 size_t path_len;
 cp_grant_id_t grant;

 __uint8_t data[36];
} mess_vfs_fs_readsuper;
typedef int _ASSERT_mess_vfs_fs_readsuper[ sizeof(mess_vfs_fs_readsuper) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;
 __off_t seek_pos;

 cp_grant_id_t grant;
 size_t nbytes;

 __uint8_t data[32];
} mess_vfs_fs_readwrite;
typedef int _ASSERT_mess_vfs_fs_readwrite[ sizeof(mess_vfs_fs_readwrite) <= 64 ? 1 : -1];

typedef struct {
 ino_t dir_old;
 ino_t dir_new;

 size_t len_old;
 size_t len_new;
 cp_grant_id_t grant_old;
 cp_grant_id_t grant_new;

 __uint8_t data[24];
} mess_vfs_fs_rename;
typedef int _ASSERT_mess_vfs_fs_rename[ sizeof(mess_vfs_fs_rename) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 size_t path_len;
 size_t mem_size;
 cp_grant_id_t grant_path;
 cp_grant_id_t grant_target;
 __uid_t uid;
 __gid_t gid;

 __uint8_t data[24];
} mess_vfs_fs_slink;
typedef int _ASSERT_mess_vfs_fs_slink[ sizeof(mess_vfs_fs_slink) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 cp_grant_id_t grant;

 __uint8_t data[44];
} mess_vfs_fs_stat;
typedef int _ASSERT_mess_vfs_fs_stat[ sizeof(mess_vfs_fs_stat) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t grant;

 __uint8_t data[52];
} mess_vfs_fs_statvfs;
typedef int _ASSERT_mess_vfs_fs_statvfs[ sizeof(mess_vfs_fs_statvfs) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;

 cp_grant_id_t grant;
 size_t path_len;

 __uint8_t data[40];
} mess_vfs_fs_unlink;
typedef int _ASSERT_mess_vfs_fs_unlink[ sizeof(mess_vfs_fs_unlink) <= 64 ? 1 : -1];

typedef struct {
 ino_t inode;
 time_t actime;
 time_t modtime;

 __uint32_t acnsec;
 __uint32_t modnsec;

 __uint8_t data[24];
} mess_vfs_fs_utime;
typedef int _ASSERT_mess_vfs_fs_utime[ sizeof(mess_vfs_fs_utime) <= 64 ? 1 : -1];

typedef struct {
 int fd0;
 int fd1;

 __uint8_t padding[48];
} mess_vfs_lc_fdpair;
typedef int _ASSERT_mess_vfs_lc_fdpair[ sizeof(mess_vfs_lc_fdpair) <= 64 ? 1 : -1];

typedef struct {
 __off_t offset;

 __uint8_t padding[48];
} mess_vfs_lc_lseek;
typedef int _ASSERT_mess_vfs_lc_lseek[ sizeof(mess_vfs_lc_lseek) <= 64 ? 1 : -1];

typedef struct {
 unsigned int len;

 __uint8_t padding[52];
} mess_vfs_lc_socklen;
typedef int _ASSERT_mess_vfs_lc_socklen[ sizeof(mess_vfs_lc_socklen) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t id;
 __devminor_t minor;

 __uint8_t padding[48];
} mess_vfs_lchardriver_cancel;
typedef int _ASSERT_mess_vfs_lchardriver_cancel[ sizeof(mess_vfs_lchardriver_cancel) <= 64 ? 1 : -1];

typedef struct {
 endpoint_t id;
 endpoint_t user;
 __devminor_t minor;
 int access;

 __uint8_t padding[40];
} mess_vfs_lchardriver_openclose;
typedef int _ASSERT_mess_vfs_lchardriver_openclose[ sizeof(mess_vfs_lchardriver_openclose) <= 64 ? 1 : -1];

typedef struct {
 __off_t pos;
 cp_grant_id_t grant;
 size_t count;
 unsigned long request;
 int flags;
 endpoint_t id;
 endpoint_t user;
 __devminor_t minor;


 __uint8_t padding[16];



} mess_vfs_lchardriver_readwrite;
typedef int _ASSERT_mess_vfs_lchardriver_readwrite[ sizeof(mess_vfs_lchardriver_readwrite) <= 64 ? 1 : -1];

typedef struct {
 __devminor_t minor;
 int ops;

 __uint8_t padding[48];
} mess_vfs_lchardriver_select;
typedef int _ASSERT_mess_vfs_lchardriver_select[ sizeof(mess_vfs_lchardriver_select) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 __int32_t sock_id;
 cp_grant_id_t grant;
 unsigned int len;
 endpoint_t user_endpt;
 int sflags;

 __uint8_t padding[32];
} mess_vfs_lsockdriver_addr;
typedef int _ASSERT_mess_vfs_lsockdriver_addr[ sizeof(mess_vfs_lsockdriver_addr) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 __int32_t sock_id;
 int level;
 int name;
 cp_grant_id_t grant;
 unsigned int len;

 __uint8_t padding[32];
} mess_vfs_lsockdriver_getset;
typedef int _ASSERT_mess_vfs_lsockdriver_getset[ sizeof(mess_vfs_lsockdriver_getset) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 __int32_t sock_id;
 unsigned long request;
 cp_grant_id_t grant;
 endpoint_t user_endpt;
 int sflags;

 __uint8_t padding[32];
} mess_vfs_lsockdriver_ioctl;
typedef int _ASSERT_mess_vfs_lsockdriver_ioctl[ sizeof(mess_vfs_lsockdriver_ioctl) <= 64 ? 1 : -1];

typedef struct {
 __int32_t sock_id;
 int ops;

 __uint8_t padding[48];
} mess_vfs_lsockdriver_select;
typedef int _ASSERT_mess_vfs_lsockdriver_select[ sizeof(mess_vfs_lsockdriver_select) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 __int32_t sock_id;
 cp_grant_id_t data_grant;
 size_t data_len;
 cp_grant_id_t ctl_grant;
 unsigned int ctl_len;
 cp_grant_id_t addr_grant;
 unsigned int addr_len;
 endpoint_t user_endpt;
 int flags;

 __uint8_t padding[16];
} mess_vfs_lsockdriver_sendrecv;
typedef int _ASSERT_mess_vfs_lsockdriver_sendrecv[ sizeof(mess_vfs_lsockdriver_sendrecv) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 __int32_t sock_id;
 int param;

 __uint8_t padding[44];
} mess_vfs_lsockdriver_simple;
typedef int _ASSERT_mess_vfs_lsockdriver_simple[ sizeof(mess_vfs_lsockdriver_simple) <= 64 ? 1 : -1];

typedef struct {
 __int32_t req_id;
 int domain;
 int type;
 int protocol;
 endpoint_t user_endpt;

 __uint8_t padding[36];
} mess_vfs_lsockdriver_socket;
typedef int _ASSERT_mess_vfs_lsockdriver_socket[ sizeof(mess_vfs_lsockdriver_socket) <= 64 ? 1 : -1];

typedef struct {
 cp_grant_id_t grant;
 size_t size;

 __uint8_t padding[48];
} mess_vfs_lsys_gcov;
typedef int _ASSERT_mess_vfs_lsys_gcov[ sizeof(mess_vfs_lsys_gcov) <= 64 ? 1 : -1];

typedef struct {
 dev_t device;
 ino_t inode;

 __uint8_t padding[40];
} mess_vfs_lsys_socketpath;
typedef int _ASSERT_mess_vfs_lsys_socketpath[ sizeof(mess_vfs_lsys_socketpath) <= 64 ? 1 : -1];

typedef struct {
 time_t atime;
 time_t mtime;
 long ansec;
 long mnsec;
 size_t len;
 char *name;
 int fd;
 int flags;

 __uint8_t padding[8];



} mess_vfs_utimens;
typedef int _ASSERT_mess_vfs_utimens[ sizeof(mess_vfs_utimens) <= 64 ? 1 : -1];

typedef struct {
 __off_t offset;
 dev_t dev;
 ino_t ino;
 endpoint_t who;
 __uint32_t vaddr;
 __uint32_t len;
 __uint32_t flags;
 __uint32_t fd;
 __uint16_t clearend;
 __uint8_t padding[8];
} mess_vm_vfs_mmap;
typedef int _ASSERT_mess_vm_vfs_mmap[ sizeof(mess_vm_vfs_mmap) <= 64 ? 1 : -1];

typedef struct {
 dev_t dev;
 __off_t dev_offset;
 __off_t ino_offset;
 ino_t ino;
 void *block;
 __uint32_t *flags_ptr;
 __uint8_t pages;
 __uint8_t flags;
 __uint8_t padding[12];
} mess_vmmcp;
typedef int _ASSERT_mess_vmmcp[ sizeof(mess_vmmcp) <= 64 ? 1 : -1];

typedef struct {
 void *addr;
 __uint8_t flags;
 __uint8_t padding[51];
} mess_vmmcp_reply;
typedef int _ASSERT_mess_vmmcp_reply[ sizeof(mess_vmmcp_reply) <= 64 ? 1 : -1];

typedef struct noxfer_message {
 endpoint_t m_source;
 int m_type;
 union {
  mess_u8 m_u8;
  mess_u16 m_u16;
  mess_u32 m_u32;
  mess_u64 m_u64;

  mess_1 m_m1;
  mess_2 m_m2;
  mess_3 m_m3;
  mess_4 m_m4;
  mess_7 m_m7;
  mess_9 m_m9;
  mess_10 m_m10;

  mess_ds_reply m_ds_reply;
  mess_ds_req m_ds_req;
  mess_fs_vfs_breadwrite m_fs_vfs_breadwrite;
  mess_fs_vfs_chmod m_fs_vfs_chmod;
  mess_fs_vfs_chown m_fs_vfs_chown;
  mess_fs_vfs_create m_fs_vfs_create;
  mess_fs_vfs_getdents m_fs_vfs_getdents;
  mess_fs_vfs_lookup m_fs_vfs_lookup;
  mess_fs_vfs_newnode m_fs_vfs_newnode;
  mess_fs_vfs_rdlink m_fs_vfs_rdlink;
  mess_fs_vfs_readsuper m_fs_vfs_readsuper;
  mess_fs_vfs_readwrite m_fs_vfs_readwrite;
  mess_i2c_li2cdriver_busc_i2c_exec m_i2c_li2cdriver_busc_i2c_exec;
  mess_i2c_li2cdriver_busc_i2c_reserve m_i2c_li2cdriver_busc_i2c_reserve;
  mess_input_linputdriver_input_conf m_input_linputdriver_input_conf;
  mess_input_linputdriver_setleds m_input_linputdriver_setleds;
  mess_input_tty_event m_input_tty_event;
  mess_krn_lsys_schedule m_krn_lsys_schedule;
  mess_krn_lsys_sys_devio m_krn_lsys_sys_devio;
  mess_krn_lsys_sys_fork m_krn_lsys_sys_fork;
  mess_krn_lsys_sys_getwhoami m_krn_lsys_sys_getwhoami;
  mess_krn_lsys_sys_irqctl m_krn_lsys_sys_irqctl;
  mess_krn_lsys_sys_times m_krn_lsys_sys_times;
  mess_krn_lsys_sys_trace m_krn_lsys_sys_trace;
  mess_krn_lsys_sys_umap m_krn_lsys_sys_umap;
  mess_krn_lsys_sys_vumap m_krn_lsys_sys_vumap;
  mess_lbdev_lblockdriver_msg m_lbdev_lblockdriver_msg;
  mess_lblockdriver_lbdev_reply m_lblockdriver_lbdev_reply;
  mess_lc_ipc_semctl m_lc_ipc_semctl;
  mess_lc_ipc_semget m_lc_ipc_semget;
  mess_lc_ipc_semop m_lc_ipc_semop;
  mess_lc_ipc_shmat m_lc_ipc_shmat;
  mess_lc_ipc_shmctl m_lc_ipc_shmctl;
  mess_lc_ipc_shmdt m_lc_ipc_shmdt;
  mess_lc_ipc_shmget m_lc_ipc_shmget;
  mess_lc_mib_sysctl m_lc_mib_sysctl;
  mess_lc_pm_exec m_lc_pm_exec;
  mess_lc_pm_exit m_lc_pm_exit;
  mess_lc_pm_getsid m_lc_pm_getsid;
  mess_lc_pm_groups m_lc_pm_groups;
  mess_lc_pm_itimer m_lc_pm_itimer;
  mess_lc_pm_mcontext m_lc_pm_mcontext;
  mess_lc_pm_priority m_lc_pm_priority;
  mess_lc_pm_ptrace m_lc_pm_ptrace;
  mess_lc_pm_reboot m_lc_pm_reboot;
  mess_lc_pm_rusage m_lc_pm_rusage;
  mess_lc_pm_setgid m_lc_pm_setgid;
  mess_lc_pm_setuid m_lc_pm_setuid;
  mess_lc_pm_sig m_lc_pm_sig;
  mess_lc_pm_sigset m_lc_pm_sigset;
  mess_lc_pm_sprof m_lc_pm_sprof;
  mess_lc_pm_sysuname m_lc_pm_sysuname;
  mess_lc_pm_time m_lc_pm_time;
  mess_lc_pm_wait4 m_lc_pm_wait4;
  mess_lc_readclock_rtcdev m_lc_readclock_rtcdev;
  mess_lc_svrctl m_lc_svrctl;
  mess_lc_vfs_chown m_lc_vfs_chown;
  mess_lc_vfs_close m_lc_vfs_close;
  mess_lc_vfs_creat m_lc_vfs_creat;
  mess_lc_vfs_fchdir m_lc_vfs_fchdir;
  mess_lc_vfs_fchmod m_lc_vfs_fchmod;
  mess_lc_vfs_fcntl m_lc_vfs_fcntl;
  mess_lc_vfs_fstat m_lc_vfs_fstat;
  mess_lc_vfs_fsync m_lc_vfs_fsync;
  mess_lc_vfs_gcov m_lc_vfs_gcov;
  mess_lc_vfs_getvfsstat m_lc_vfs_getvfsstat;
  mess_lc_vfs_ioctl m_lc_vfs_ioctl;
  mess_lc_vfs_link m_lc_vfs_link;
  mess_lc_vfs_listen m_lc_vfs_listen;
  mess_lc_vfs_lseek m_lc_vfs_lseek;
  mess_lc_vfs_mknod m_lc_vfs_mknod;
  mess_lc_vfs_mount m_lc_vfs_mount;
  mess_lc_vfs_path m_lc_vfs_path;
  mess_lc_vfs_pipe2 m_lc_vfs_pipe2;
  mess_lc_vfs_readlink m_lc_vfs_readlink;
  mess_lc_vfs_readwrite m_lc_vfs_readwrite;
  mess_lc_vfs_select m_lc_vfs_select;
  mess_lc_vfs_sendrecv m_lc_vfs_sendrecv;
  mess_lc_vfs_shutdown m_lc_vfs_shutdown;
  mess_lc_vfs_sockaddr m_lc_vfs_sockaddr;
  mess_lc_vfs_socket m_lc_vfs_socket;
  mess_lc_vfs_sockmsg m_lc_vfs_sockmsg;
  mess_lc_vfs_sockopt m_lc_vfs_sockopt;
  mess_lc_vfs_stat m_lc_vfs_stat;
  mess_lc_vfs_statvfs1 m_lc_vfs_statvfs1;
  mess_lc_vfs_truncate m_lc_vfs_truncate;
  mess_lc_vfs_umask m_lc_vfs_umask;
  mess_lc_vfs_umount m_lc_vfs_umount;
  mess_lc_vm_brk m_lc_vm_brk;
  mess_lc_vm_getphys m_lc_vm_getphys;
  mess_lc_vm_shm_unmap m_lc_vm_shm_unmap;
  mess_lchardriver_vfs_reply m_lchardriver_vfs_reply;
  mess_lchardriver_vfs_sel1 m_lchardriver_vfs_sel1;
  mess_lchardriver_vfs_sel2 m_lchardriver_vfs_sel2;
  mess_lexec_pm_exec_new m_lexec_pm_exec_new;
  mess_li2cdriver_i2c_busc_i2c_exec m_li2cdriver_i2c_busc_i2c_exec;
  mess_li2cdriver_i2c_busc_i2c_reserve m_li2cdriver_i2c_busc_i2c_reserve;
  mess_linputdriver_input_event m_linputdriver_input_event;
  mess_lsockdriver_vfs_accept_reply
      m_lsockdriver_vfs_accept_reply;
  mess_lsockdriver_vfs_recv_reply
      m_lsockdriver_vfs_recv_reply;
  mess_lsockdriver_vfs_reply m_lsockdriver_vfs_reply;
  mess_lsockdriver_vfs_select_reply
      m_lsockdriver_vfs_select_reply;
  mess_lsockdriver_vfs_socket_reply
      m_lsockdriver_vfs_socket_reply;
  mess_lsys_fi_ctl m_lsys_fi_ctl;
  mess_lsys_fi_reply m_lsys_fi_reply;
  mess_lsys_getsysinfo m_lsys_getsysinfo;
  mess_lsys_krn_readbios m_lsys_krn_readbios;
  mess_lsys_kern_safecopy m_lsys_kern_safecopy;
  mess_lsys_krn_schedctl m_lsys_krn_schedctl;
  mess_lsys_krn_schedule m_lsys_krn_schedule;
  mess_lsys_krn_sys_abort m_lsys_krn_sys_abort;
  mess_lsys_krn_sys_clear m_lsys_krn_sys_clear;
  mess_lsys_krn_sys_copy m_lsys_krn_sys_copy;
  mess_lsys_krn_sys_devio m_lsys_krn_sys_devio;
  mess_lsys_krn_sys_diagctl m_lsys_krn_sys_diagctl;
  mess_lsys_krn_sys_exec m_lsys_krn_sys_exec;
  mess_lsys_krn_sys_fork m_lsys_krn_sys_fork;
  mess_lsys_krn_sys_getinfo m_lsys_krn_sys_getinfo;
  mess_lsys_krn_sys_getmcontext m_lsys_krn_sys_getmcontext;
  mess_lsys_krn_sys_iopenable m_lsys_krn_sys_iopenable;
  mess_lsys_krn_sys_irqctl m_lsys_krn_sys_irqctl;
  mess_lsys_krn_sys_memset m_lsys_krn_sys_memset;
  mess_lsys_krn_sys_privctl m_lsys_krn_sys_privctl;
  mess_lsys_krn_sys_sdevio m_lsys_krn_sys_sdevio;
  mess_lsys_krn_sys_setalarm m_lsys_krn_sys_setalarm;
  mess_lsys_krn_sys_setgrant m_lsys_krn_sys_setgrant;
  mess_lsys_krn_sys_setmcontext m_lsys_krn_sys_setmcontext;
  mess_lsys_krn_sys_settime m_lsys_krn_sys_settime;
  mess_lsys_krn_sys_sprof m_lsys_krn_sys_sprof;
  mess_lsys_krn_sys_statectl m_lsys_krn_sys_statectl;
  mess_lsys_krn_sys_stime m_lsys_krn_sys_stime;
  mess_lsys_krn_sys_times m_lsys_krn_sys_times;
  mess_lsys_krn_sys_trace m_lsys_krn_sys_trace;
  mess_lsys_krn_sys_umap m_lsys_krn_sys_umap;
  mess_lsys_krn_sys_vdevio m_lsys_krn_sys_vdevio;
  mess_lsys_krn_sys_vumap m_lsys_krn_sys_vumap;
  mess_lsys_kern_vsafecopy m_lsys_kern_vsafecopy;
  mess_lsys_mib_register m_lsys_mib_register;
  mess_lsys_mib_reply m_lsys_mib_reply;
  mess_lsys_pci_busc_get_bar m_lsys_pci_busc_get_bar;
  mess_lsys_pm_getepinfo m_lsys_pm_getepinfo;
  mess_lsys_pm_getprocnr m_lsys_pm_getprocnr;
  mess_lsys_pm_proceventmask m_lsys_pm_proceventmask;
  mess_lsys_pm_srv_fork m_lsys_pm_srv_fork;
  mess_lsys_sched_scheduling_start m_lsys_sched_scheduling_start;
  mess_lsys_sched_scheduling_stop m_lsys_sched_scheduling_stop;
  mess_lsys_tty_fkey_ctl m_lsys_tty_fkey_ctl;
  mess_lsys_vfs_copyfd m_lsys_vfs_copyfd;
  mess_lsys_vfs_mapdriver m_lsys_vfs_mapdriver;
  mess_lsys_vfs_socketpath m_lsys_vfs_socketpath;
  mess_lsys_vm_getref m_lsys_vm_getref;
  mess_lsys_vm_info m_lsys_vm_info;
  mess_lsys_vm_map_phys m_lsys_vm_map_phys;
  mess_lsys_vm_rusage m_lsys_vm_rusage;
  mess_lsys_vm_unmap_phys m_lsys_vm_unmap_phys;
  mess_lsys_vm_update m_lsys_vm_update;
  mess_lsys_vm_vmremap m_lsys_vm_vmremap;
  mess_mib_lc_sysctl m_mib_lc_sysctl;
  mess_mib_lsys_call m_mib_lsys_call;
  mess_mib_lsys_info m_mib_lsys_info;
  mess_mmap m_mmap;
  mess_ndev_netdriver_init m_ndev_netdriver_init;
  mess_ndev_netdriver_conf m_ndev_netdriver_conf;
  mess_ndev_netdriver_transfer m_ndev_netdriver_transfer;
  mess_ndev_netdriver_status_reply m_ndev_netdriver_status_reply;
  mess_netdriver_ndev_init_reply m_netdriver_ndev_init_reply;
  mess_netdriver_ndev_reply m_netdriver_ndev_reply;
  mess_netdriver_ndev_status m_netdriver_ndev_status;
  mess_net_netdrv_dl_conf m_net_netdrv_dl_conf;
  mess_net_netdrv_dl_getstat_s m_net_netdrv_dl_getstat_s;
  mess_net_netdrv_dl_readv_s m_net_netdrv_dl_readv_s;
  mess_net_netdrv_dl_writev_s m_net_netdrv_dl_writev_s;
  mess_netdrv_net_dl_conf m_netdrv_net_dl_conf;
  mess_netdrv_net_dl_task m_netdrv_net_dl_task;
  mess_notify m_notify;
  mess_pci_lsys_busc_get_bar m_pci_lsys_busc_get_bar;
  mess_pm_lc_getgid m_pm_lc_getgid;
  mess_pm_lc_getpid m_pm_lc_getpid;
  mess_pm_lc_getuid m_pm_lc_getuid;
  mess_pm_lc_ptrace m_pm_lc_ptrace;
  mess_pm_lc_sigset m_pm_lc_sigset;
  mess_pm_lc_time m_pm_lc_time;
  mess_pm_lc_wait4 m_pm_lc_wait4;
  mess_pm_lexec_exec_new m_pm_lexec_exec_new;
  mess_pm_lsys_getepinfo m_pm_lsys_getepinfo;
  mess_pm_lsys_getprocnr m_pm_lsys_getprocnr;
  mess_pm_lsys_proc_event m_pm_lsys_proc_event;
  mess_pm_lsys_sigs_signal m_pm_lsys_sigs_signal;
  mess_pm_sched_scheduling_set_nice m_pm_sched_scheduling_set_nice;
  mess_pty_ptyfs_req m_pty_ptyfs_req;
  mess_ptyfs_pty_name m_ptyfs_pty_name;
  mess_readclock_lc_rtcdev m_readclock_lc_rtcdev;
  mess_rs_init m_rs_init;
  mess_rs_pm_exec_restart m_rs_pm_exec_restart;
  mess_rs_pm_srv_kill m_rs_pm_srv_kill;
  mess_rs_req m_rs_req;
  mess_rs_update m_rs_update;
  mess_sched_lsys_scheduling_start m_sched_lsys_scheduling_start;
  mess_sigcalls m_sigcalls;
  mess_tty_lsys_fkey_ctl m_tty_lsys_fkey_ctl;
  mess_vfs_fs_breadwrite m_vfs_fs_breadwrite;
  mess_vfs_fs_chmod m_vfs_fs_chmod;
  mess_vfs_fs_chown m_vfs_fs_chown;
  mess_vfs_fs_create m_vfs_fs_create;
  mess_vfs_fs_flush m_vfs_fs_flush;
  mess_vfs_fs_ftrunc m_vfs_fs_ftrunc;
  mess_vfs_fs_getdents m_vfs_fs_getdents;
  mess_vfs_fs_inhibread m_vfs_fs_inhibread;
  mess_vfs_fs_link m_vfs_fs_link;
  mess_vfs_fs_lookup m_vfs_fs_lookup;
  mess_vfs_fs_mkdir m_vfs_fs_mkdir;
  mess_vfs_fs_mknod m_vfs_fs_mknod;
  mess_vfs_fs_mountpoint m_vfs_fs_mountpoint;
  mess_vfs_fs_new_driver m_vfs_fs_new_driver;
  mess_vfs_fs_newnode m_vfs_fs_newnode;
  mess_vfs_fs_putnode m_vfs_fs_putnode;
  mess_vfs_fs_rdlink m_vfs_fs_rdlink;
  mess_vfs_fs_readsuper m_vfs_fs_readsuper;
  mess_vfs_fs_readwrite m_vfs_fs_readwrite;
  mess_vfs_fs_rename m_vfs_fs_rename;
  mess_vfs_fs_slink m_vfs_fs_slink;
  mess_vfs_fs_stat m_vfs_fs_stat;
  mess_vfs_fs_statvfs m_vfs_fs_statvfs;
  mess_vfs_fs_unlink m_vfs_fs_unlink;
  mess_vfs_fs_utime m_vfs_fs_utime;
  mess_vfs_lc_fdpair m_vfs_lc_fdpair;
  mess_vfs_lc_lseek m_vfs_lc_lseek;
  mess_vfs_lc_socklen m_vfs_lc_socklen;
  mess_vfs_lchardriver_cancel m_vfs_lchardriver_cancel;
  mess_vfs_lchardriver_openclose m_vfs_lchardriver_openclose;
  mess_vfs_lchardriver_readwrite m_vfs_lchardriver_readwrite;
  mess_vfs_lchardriver_select m_vfs_lchardriver_select;
  mess_vfs_lsockdriver_addr m_vfs_lsockdriver_addr;
  mess_vfs_lsockdriver_getset m_vfs_lsockdriver_getset;
  mess_vfs_lsockdriver_ioctl m_vfs_lsockdriver_ioctl;
  mess_vfs_lsockdriver_select m_vfs_lsockdriver_select;
  mess_vfs_lsockdriver_sendrecv m_vfs_lsockdriver_sendrecv;
  mess_vfs_lsockdriver_simple m_vfs_lsockdriver_simple;
  mess_vfs_lsockdriver_socket m_vfs_lsockdriver_socket;
  mess_vfs_lsys_gcov m_vfs_lsys_gcov;
  mess_vfs_lsys_socketpath m_vfs_lsys_socketpath;
  mess_vfs_utimens m_vfs_utimens;
  mess_vm_vfs_mmap m_vm_vfs_mmap;
  mess_vmmcp m_vmmcp;
  mess_vmmcp_reply m_vmmcp_reply;

  __uint8_t size[64];
 };
} message _Alignas(16);



typedef int _ASSERT_message[ sizeof(message) >= (8 + 64) ? 1 : -1];
# 3008 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
typedef struct asynmsg
{
 unsigned flags;
 endpoint_t dst;
 int result;
 message msg;
} asynmsg_t;
# 3027 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
int _ipc_send_intr(endpoint_t dest, message *m_ptr);
int _ipc_receive_intr(endpoint_t src, message *m_ptr, int *status_ptr);
int _ipc_sendrec_intr(endpoint_t src_dest, message *m_ptr);
int _ipc_sendnb_intr(endpoint_t dest, message *m_ptr);
int _ipc_notify_intr(endpoint_t dest);
int _ipc_senda_intr(asynmsg_t *table, size_t count);

int _do_kernel_call_intr(message *m_ptr);

int ipc_minix_kerninfo(struct minix_kerninfo **);
# 3049 "C:/Users/VIC/gergios/minix/include\\minix/ipc.h"
struct minix_ipcvecs {
 int (*send)(endpoint_t dest, message *m_ptr);
 int (*receive)(endpoint_t src, message *m_ptr, int *st);
 int (*sendrec)(endpoint_t src_dest, message *m_ptr);
 int (*sendnb)(endpoint_t dest, message *m_ptr);
 int (*notify)(endpoint_t dest);
 int (*_do_kernel_call)(message *m_ptr);
 int (*senda)(asynmsg_t *table, size_t count);
};



extern struct minix_ipcvecs _minix_ipcvecs;

static inline int _ipc_send(endpoint_t dest, message *m_ptr)
{
 return _minix_ipcvecs.send(dest, m_ptr);
}

static inline int _ipc_receive(endpoint_t src, message *m_ptr, int *st)
{
 return _minix_ipcvecs.receive(src, m_ptr, st);
}

static inline int _ipc_sendrec(endpoint_t src_dest, message *m_ptr)
{
 return _minix_ipcvecs.sendrec(src_dest, m_ptr);
}

static inline int _ipc_sendnb(endpoint_t dest, message *m_ptr)
{
 return _minix_ipcvecs.sendnb(dest, m_ptr);
}

static inline int _ipc_notify(endpoint_t dest)
{
 return _minix_ipcvecs.notify(dest);
}

static inline int _do_kernel_call(message *m_ptr)
{
 return _minix_ipcvecs._do_kernel_call(m_ptr);
}

static inline int _ipc_senda(asynmsg_t *table, size_t count)
{
 return _minix_ipcvecs.senda(table, count);
}
# 2 "test_ipc.c" 2

