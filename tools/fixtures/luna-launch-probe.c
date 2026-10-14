/* Native musl acceptance payload, never installed by the production builder. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <stddef.h>
#include <unistd.h>

static void require(int condition, int code) { if (!condition) _exit(code); }
static void write_text(const char *path, const char *text) {
    FILE *file = fopen(path, "w"); require(file != NULL, 50);
    require(fputs(text, file) >= 0 && fflush(file) == 0, 51);
    require(fsync(fileno(file)) == 0 && fclose(file) == 0, 52);
}
static void common(void) {
    uid_t real, effective, saved; gid_t greal, geffective, gsaved;
    require(getresuid(&real,&effective,&saved)==0 && real==1000 && effective==1000 && saved==1000, 41);
    require(getresgid(&greal,&geffective,&gsaved)==0 && greal==1000 && geffective==1000 && gsaved==1000, 42);
    require(prctl(PR_GET_NO_NEW_PRIVS,0,0,0,0)==1, 43);
    require(getpid()!=1 && getenv("LD_LIBRARY_PATH")==NULL && getenv("LD_PRELOAD")==NULL, 44);
    require(getenv("HOME") && strcmp(getenv("HOME"),"/data")==0, 45);
    require(access("/.luna/data",F_OK)!=0 && access("/usr/bin/busybox",F_OK)!=0 && access("/etc/shadow",F_OK)!=0, 46);
    for(int fd=3;fd<128;fd++) require(fcntl(fd,F_GETFD)==-1 && errno==EBADF, 47);
    require(open("/bin/probe",O_WRONLY)<0, 48);
}
static void endpoint(struct sockaddr_un *address, int abstract) {
    memset(address,0,sizeof(*address)); address->sun_family=AF_UNIX;
    if(abstract) memcpy(address->sun_path+1,"luna-launch-external",20);
    else strcpy(address->sun_path,"/data/external.sock");
}
static void serve_ipc(void) {
    require(getuid()==1000,70);
    struct sockaddr_un named, abstract; endpoint(&named,0); endpoint(&abstract,1);
    strcpy(named.sun_path,"/data/users/luna/data/org.luna.launch-probe/external.sock");
    int a=socket(AF_UNIX,SOCK_STREAM|SOCK_CLOEXEC,0), b=socket(AF_UNIX,SOCK_STREAM|SOCK_CLOEXEC,0);
    require(a>=0 && b>=0,71); unlink(named.sun_path);
    require(bind(a,(struct sockaddr*)&named,sizeof(named))==0 && listen(a,4)==0,72);
    require(bind(b,(struct sockaddr*)&abstract,offsetof(struct sockaddr_un,sun_path)+21)==0 && listen(b,4)==0,73);
    write_text("/data/users/luna/data/org.luna.launch-probe/ipc-ready","ready\n");
    for(;;) pause();
}
static void denied_ipc(void) {
    for(int abstract=0;abstract<2;abstract++) {
        struct sockaddr_un address; endpoint(&address,abstract);
        int fd=socket(AF_UNIX,SOCK_STREAM|SOCK_CLOEXEC,0); require(fd>=0,74);
        int length=abstract?(int)offsetof(struct sockaddr_un,sun_path)+21:(int)sizeof(address);
        int result=connect(fd,(struct sockaddr*)&address,(socklen_t)length);
        require(result<0 && (errno==EACCES || errno==EPERM),75); close(fd);
    }
}
int main(int argc, char **argv) {
    if(argc==2 && !strcmp(argv[1],"serve-ipc")) { serve_ipc(); return 0; }
    common(); require(argc==2, 49);
    if(!strcmp(argv[1],"ipc-deny")) { denied_ipc(); return 0; }
    if(!strcmp(argv[1],"write")) {
        write_text("/data/proof.txt","uid=1000 gid=1000 saved=1000 no_new_privs=1 isolated=yes fds=closed\n");
        write_text("/config/proof.txt","persistent-config\n");
        return 17;
    }
    if(!strcmp(argv[1],"read")) {
        FILE *data=fopen("/data/proof.txt","r"), *config=fopen("/config/proof.txt","r");
        require(data && config,53);
        char text[256]; require(fgets(text,sizeof(text),data) && strstr(text,"isolated=yes"),54);
        require(fgets(text,sizeof(text),config) && !strcmp(text,"persistent-config\n"),55);
        fclose(data); fclose(config); return 0;
    }
    if(!strcmp(argv[1],"signal")) { raise(SIGTERM); return 56; }
    if(!strcmp(argv[1],"detached") || !strcmp(argv[1],"hold")) {
        int ready[2]; require(pipe(ready)==0,57);
        pid_t child=fork(); require(child>=0,58);
        if(child==0) {
            close(ready[0]); require(setsid()>0,59);
            char text[64]; snprintf(text,sizeof(text),"%d\n",getpid());
            write_text(!strcmp(argv[1],"hold")?"/data/hold.pid":"/data/detached.pid",text);
            require(write(ready[1],"x",1)==1,60); close(ready[1]);
            for(;;) pause();
        }
        close(ready[1]); char byte; require(read(ready[0],&byte,1)==1,61); close(ready[0]);
        if(!strcmp(argv[1],"hold")) { for(;;) pause(); }
        return 17;
    }
    return 62;
}
