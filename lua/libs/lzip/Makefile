T=lzip

UNAME            ?= $(shell uname)
DESTDIR          ?= /
PKG_CONFIG       ?= pkg-config
INSTALL          ?= install
RM               ?= rm
LUA_IMPL         ?= luajit
CC               ?= $(MAC_ENV) g++

LUA_VERSION       = $(shell $(PKG_CONFIG) --print-provides --silence-errors $(LUA_IMPL))
ifeq ($(LUA_VERSION),)
LUA_CMOD         ?= /usr/lib/lua/5.1
LUA_LMOD         ?= /usr/share/lua/5.1
LIBDIR           ?= /usr/lib
LUA_INC          ?= /usr/include
ZLIB_LIBS         = -L/usr/lib -lz
else
LUA_CMOD         ?= $(shell $(PKG_CONFIG) --variable INSTALL_CMOD $(LUA_IMPL))
LUA_LMOD         ?= $(shell $(PKG_CONFIG) --variable INSTALL_LMOD $(LUA_IMPL))
LIBDIR           ?= $(shell $(PKG_CONFIG) --variable libdir $(LUA_IMPL))
LUA_INC          ?= $(shell $(PKG_CONFIG) --variable includedir $(LUA_IMPL))
LUA_LIBS          = $(shell $(PKG_CONFIG) --libs $(LUA_IMPL))
ZLIB_LIBS         = $(shell $(PKG_CONFIG) --libs zlib)
endif

ifeq ($(UNAME), Linux)
OS_FLAGS         ?= -shared
endif
ifeq ($(UNAME), Darwin)
OS_FLAGS         ?= -bundle -undefined dynamic_lookup
MAC_ENV          ?= env MACOSX_DEPLOYMENT_TARGET='10.3'
endif

ifneq ($(DEBUG),)
DBG               = -ggdb
endif

ifeq ($(DEV),)
WARN              = -Wall -Wno-unused-value
else
WARN              = -Wall -W -Waggregate-return -Wcast-align -Wmissing-prototypes -Wnested-externs -Wshadow -Wwrite-strings -pedantic
endif

INCLUDES          = -I$(LUA_INC)
DEFINES           =
LIBS              = $(ZLIB_LIBS) -lstdc++

COMMONFLAGS       = -O2 -g -pipe -fPIC $(OS_FLAGS) $(DBG)
LF                = $(LIBS) $(LDFLAGS)
CF                = $(INCLUDES) $(DEFINES) $(COMMONFLAGS) $(WARN) -DPTHREADS $(CFLAGS)

SRCS              = src/*.cpp
OBJS              = $(subst src/,,$(subst .c,.o,$(SRCS)))

BIN               = $(T).so
STATIC_LIB        = $(T).a

all: $(BIN)

$(BIN): $(SRCS)
	$(CC) $(CF) -o $@ $^ $(LF)

$(OBJS): $(SRCS)
	$(CC) $(CF) -c $^ $(LF)

$(STATIC_LIB): $(OBJS)
	ar rcs $@ $^

install: all
	$(INSTALL) -d $(DESTDIR)$(LUA_CMOD)
	$(INSTALL) $(BIN) $(DESTDIR)$(LUA_CMOD)

clean:
	$(RM) -f $(BIN) $(OBJS) $(STATIC_LIB)
