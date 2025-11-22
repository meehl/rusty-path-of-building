#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "zlib.h"

#ifdef _WIN32
#define LZIP_EXPORT __declspec(dllexport)
#else
#define LZIP_EXPORT
#endif

extern "C" {
#include "lua.h"
#include "lauxlib.h"
#include "lualib.h"
#include "luajit.h"
}

typedef unsigned char byte;
typedef unsigned int dword;
typedef unsigned long lword;

char* AllocString(char* st)
{
	if (st == NULL) {
		return NULL;
	}

	int aslen = strlen(st) + 1;
	char* al = new char[aslen];
	strcpy(al, st);
	return al;
}

char* AllocStringLen(int len)
{
	char* al = new char[len+1];
	al[len] = 0;
	return al;
}

void FreeString(char* st)
{
	if (st) delete st;
}

// =============
// Configuration
// =============

static const int FS_INFLATE_BUFFER = 65536;

// ==========================
// Zip File format structures
// ==========================

#pragma pack(push, 1)

struct zf_cdHeader_s {
	int		sig;	// 0x02014b50 (50,4B,01,02)
	short	ver_made;
	short	ver_ext;
	short	flag;
	short	method;
	short	ftime;
	short	fdate;
	int		crc32;
	lword	szComp;
	lword	szUcomp;
	short	szName;
	short	szExtra;
	short	szComm;
	short	nDisk;
	short	attInt;
	int		attExt;
	int		hOffset;
};

struct zf_localHeader_s {
	int		sig;	// 0x04034b50 (50,4B,03,04)
	short	ver_ext;
	short	flag;
	short	method;
	short	ftime;
	short	fdate;
	int		crc32;
	lword	szComp;
	lword	szUcomp;
	short	szName;
	short	szExtra;
};

struct zf_dataDesc_s {
	int		crc32;
	lword	szComp;
	lword	szUcomp;
};

struct zf_cdEnd_s {
	int		sig;	// 0x06054b50 (50,4B,05,06)
	short	nDisk;
	short	stDisk;
	short	noeDisk;
	short	noeTotal;
	int		szTotal;
	int		cdOffset;
	short	szComm;
};

#pragma pack(pop)

// =========================
// Zip File format constants
// =========================

static const int ZF_MAXVER = 20;

static const int ZF_GPBF_ENC = 0x0001;
static const int ZF_GPBF_DD  = 0x0008;

static const int ZF_METHOD_STORE = 0;
static const int ZF_METHOD_DEFLATE = 8;

static const int ZF_SIG_CDHEADER = 0x02014B50;
static const int ZF_SIG_LOCALHEADER = 0x04034B50;
static const int ZF_SIG_CDEND = 0x06054B50;

// ===============
// ZLib Alloc/Free
// ===============

static void* ZF_Alloc(void* unused, dword num, dword size)
{
	return new byte[num*size];
}

static void ZF_Free(void* unused, void* ptr)
{
	delete (byte*)ptr;
}

// =======
// Classes
// =======

struct fs_fileInfo_s {
	FILE*	f;		// Zip file
	char*	name;	// Filename
	int		fo;		// File offset
	int		szU;	// Uncompressed size
	int		szC;	// Compressed size, 0 if not compressed
};

// ================
// fsInflator class
// ================

class fs_inflator_c {
public:
	fs_inflator_c(fs_fileInfo_s* i);
	~fs_inflator_c();
	int	Inflate(byte* buf, int len);// Inflate

private:
	int		pC;			// Read pointer
	byte*	ibuf;		// Input buffer
	int		ibuflen;	// Input buffer length
	fs_fileInfo_s* i;	// Info structure
	z_stream* zst;		// ZLib stream

	void	FillInput();// Fill decompression input
};

fs_inflator_c::fs_inflator_c(fs_fileInfo_s* info)
{
	i = info;

	// Initialise stream
	zst = new z_stream;
	memset(zst, 0, sizeof(z_stream));
	zst->zalloc = ZF_Alloc;
	zst->zfree = ZF_Free;

	// Initialise decompression input
	pC = 0;
	if (i->szC < FS_INFLATE_BUFFER) {
		ibuflen = i->szC;
	} else {
		ibuflen = FS_INFLATE_BUFFER;
	}
	ibuf = new byte[ibuflen];
	FillInput();

	// Start decompressor
	inflateInit2(zst, -MAX_WBITS);	
}

fs_inflator_c::~fs_inflator_c()
{
	// Stop decompressor
	inflateEnd(zst);

	// Release stream and input buffer
	delete ibuf;
	delete zst;
}

void fs_inflator_c::FillInput()
{
	// Relocate any remaining input
	int remin = zst->avail_in;
	if (remin) {
		memcpy(ibuf, zst->next_in, remin); 
	}

	// Get reading amount
	int rdmax = ibuflen - remin;
	int rd = i->szC - pC;
	if (rd > rdmax) {
		rd = rdmax;
	}

	// Read into buffer
	if (rd) {
		fseek(i->f, i->fo + pC, SEEK_SET);
		fread(ibuf + remin, rd, 1, i->f);
	}

	// Update stream
	zst->next_in = ibuf;
	zst->avail_in+= rd;
	pC+= rd;
}

int fs_inflator_c::Inflate(byte* out, int len)
{
	// Prepare stream
	zst->next_out = out;
	zst->avail_out = len;

	while (1) {
		// Decompress some data
		int r = inflate(zst, Z_SYNC_FLUSH);

		switch (r) {
		case Z_OK:	
			// Progress made, check if we're done
			if (zst->avail_out == 0) {
				return len;
			}
			break;
		case Z_BUF_ERROR:
			// No progress made, so give it more input
			FillInput();
			break;
		default:
			// We got an error
		case Z_STREAM_END:
			// Reached the end of the stream
			return len - zst->avail_out;
		}
	}
}

// =============
// fsFile class
// =============

enum vf_state_e {
	VF_OPEN	=0x01,	// Open
	VF_SEEK	=0x04	// Seek enabled
};

class fs_file_c {
public:
	fs_file_c(fs_fileInfo_s* i);
	~fs_file_c();
	void	Seek(int pos, int mode);	// Seek
	int		Read(byte* buf, int len);	// Read data
	int		Length();					// Return the file length
	int		Tell();						// Return the read pointer

private:
	fs_fileInfo_s* i;	// Info structure
	int		pU;			// Read pointer
	fs_inflator_c* inf;	// Inflator
};

fs_file_c::fs_file_c(fs_fileInfo_s* info)
{
	i = info;

	// Create inflator
	if (i->szC) {
		inf = new fs_inflator_c(i);
	} else {
		inf = NULL;
	}
	
	// Set position
	pU = 0;
}

fs_file_c::~fs_file_c()
{
	delete inf;
}

void fs_file_c::Seek(int pos, int mode)
{
	int seekto;

	switch (mode) {
	case SEEK_SET:
		seekto = pos;
		break;
	case SEEK_CUR:
		seekto = pU + pos;
		break;
	case SEEK_END:
		seekto = i->szU - pos;
		break;
	default:
		return;
	}

	if (i->szC) {
		int diff = seekto - pU;
		if (diff > 0) {
			// Seek forward
			byte* sk = new byte[diff];
			Read(sk, diff);
			delete[] sk;
		} else if (diff < 0) {
			// Restart reading
			pU = 0;
			// Recreate inflator
			delete inf;
			inf = new fs_inflator_c(i);
			if (seekto) {
				// Seek forward
				byte* sk = new byte[seekto];
				Read(sk, seekto);
				delete[] sk;
			}
		}
	} else {
		pU = seekto;
		if (pU < 0) {
			pU = 0;
		} else if (pU > i->szU) {
			pU = i->szU;
		}
	}
}

int fs_file_c::Read(byte* out, int len)
{
	// Determine read length
	int rp = len + pU;
	if (rp > i->szU) len = i->szU - pU;
	if (len == 0) {
		return 0;
	}

	if (i->szC) {
		// Decompress
		len = inf->Inflate(out, len);
	} else {
		// Raw read
		fseek(i->f, i->fo + pU, SEEK_SET);
		len = fread(out, 1, len, i->f);
	}

	pU+= len;
	return len;
}

int fs_file_c::Length()
{
	return i->szU;
}

int fs_file_c::Tell()
{
	return pU;
}

// =============
// zipFile class
// =============

class fs_zipFile_c {
public:
	fs_zipFile_c(const char* zname);
	~fs_zipFile_c();

	int		numFiles;
	fs_fileInfo_s** files;
private:
	FILE*	zf;				// File handle
	int		filesSz;

	int		GetSignature();	// Get next signature
};

int fs_zipFile_c::GetSignature()
{
	int sig = 0;
	if (fread(&sig, 4, 1, zf) == 0) {
		return 0;
	}
	fseek(zf, -4, SEEK_CUR);
	return sig;
}

fs_zipFile_c::fs_zipFile_c(const char* zname)
{
	numFiles = 0;
	filesSz = 32;
	files = new fs_fileInfo_s*[filesSz];

	int sig;

	// Open and lock file
	zf = fopen(zname, "rb");
	if (zf == NULL) {
		return;
	}

	// Check signature
	sig = GetSignature();
	if (strncmp((char*)&sig, "Rar", 3) == 0) {
		fclose(zf);
		return;
	}

	// Read local headers
	while (sig == ZF_SIG_LOCALHEADER) {
		zf_localHeader_s lh;
		if (fread(&lh, sizeof(lh), 1, zf) == 0) {
			fclose(zf);
			return;
		}

		if (lh.szName) {
			// Read filename
			char* fname = AllocStringLen(lh.szName);
			fread(fname, lh.szName, 1, zf); 

			// Add it in if it is a file
			if (fname[lh.szName-1] != '/') {
				// Fill in info
				fs_fileInfo_s* fi = new fs_fileInfo_s;
				fi->f = zf;
				fi->name = fname;
				fi->fo = ftell(zf) + lh.szExtra;
				fi->szU = lh.szUcomp;
				fi->szC = (lh.method == ZF_METHOD_DEFLATE)? lh.szComp : 0;

				// Record file
				if (numFiles == filesSz) {
					filesSz<<= 1;
					files = (fs_fileInfo_s**)realloc(files, sizeof(fs_fileInfo_s*)*filesSz);
				}
				files[numFiles++] = fi;
			} else {
				FreeString(fname);
			}
			
			// Skip the extra information and file data
			fseek(zf, lh.szExtra + lh.szComp, SEEK_CUR);
		}

		sig = GetSignature();
	}
}

fs_zipFile_c::~fs_zipFile_c()
{
	fclose(zf);
	for (int f = 0; f < numFiles; f++) {
		delete files[f]->name;
		delete files[f];
	}
	delete files;
}

// =============
// Lua Interface
// =============

static int IsUserData(lua_State* L, int index, const char* metaName)
{
	if (lua_type(L, index) != LUA_TUSERDATA || lua_getmetatable(L, index) == 0) {
		return 0;
	}
	lua_getfield(L, lua_upvalueindex(1), metaName);
	int ret = lua_rawequal(L, -2, -1);
	lua_pop(L, 2);
	return ret;
}

struct lzip_s {
	fs_zipFile_c* zipFile;
};
static lzip_s* GetZip(lua_State* L, const char* method, bool valid)
{
	if ( !IsUserData(L, 1, "zipMeta") ) {
		luaL_error(L, "zip:%s() must be used on a zip handle", method);
	}
	lzip_s* lz = (lzip_s*)lua_touserdata(L, 1);
	lua_remove(L, 1);
	if (valid && lz->zipFile == NULL) {
		luaL_error(L, "zip:%s(): zip handle is closed");
	}
	return lz;
}

struct lzipFile_s {
	fs_file_c* file;
};
static lzipFile_s* GetZipFile(lua_State* L, const char* method, bool valid)
{
	if ( !IsUserData(L, 1, "zipFileMeta") ) {
		luaL_error(L, "zipFile:%s() must be used on a zip file handle", method);
	}
	lzipFile_s* lzf = (lzipFile_s*)lua_touserdata(L, 1);
	lua_remove(L, 1);
	if (valid && lzf->file == NULL) {
		luaL_error(L, "zipFile:%s(): zip file handle is closed");
	}
	return lzf;
}

static int l_open(lua_State* L)
{
	int n = lua_gettop(L);
	if (n < 1 || !lua_isstring(L, 1)) {
		luaL_error(L, "Usage: lzip.open(fileName)");
	}
	fs_zipFile_c* zipFile = new fs_zipFile_c(lua_tostring(L, 1));
	if (zipFile->numFiles == 0) {
		delete zipFile;
		return 0;
	}
	lzip_s* lz = (lzip_s*)lua_newuserdata(L, sizeof(lzip_s));
	lz->zipFile = zipFile;
	lua_pushvalue(L, lua_upvalueindex(2));
	lua_setmetatable(L, -2);
	return 1;
}

static int l_zip_Close(lua_State* L)
{
	lzip_s* lz = GetZip(L, "Closea", false);
	delete lz->zipFile;
	lz->zipFile = NULL;
	return 0;
}

static int l_zip_GetNumFiles(lua_State* L)
{
	lzip_s* lz = GetZip(L, "GetNumFiles", true);
	lua_pushinteger(L, lz->zipFile->numFiles);
	return 1;
}

static int l_zip_GetFileName(lua_State* L)
{
	lzip_s* lz = GetZip(L, "GetFileName", true);
	int n = lua_gettop(L);
	if (n < 1 || !lua_isnumber(L, 1)) {
		luaL_error(L, "Usage: zip:GetFileName(index)");
	}
	int index = lua_tointeger(L, 1);
	if (index < 1 || index > lz->zipFile->numFiles) {
		return 0;
	}
	lua_pushstring(L, lz->zipFile->files[index - 1]->name);
	return 1;
}

static int l_zip_GetFileSize(lua_State* L)
{
	lzip_s* lz = GetZip(L, "GetFileSize", true);
	int n = lua_gettop(L);
	if (n < 1 || !lua_isnumber(L, 1)) {
		luaL_error(L, "Usage: zip:GetFileSize(index)");
	}
	int index = lua_tointeger(L, 1);
	if (index < 1 || index > lz->zipFile->numFiles) {
		return 0;
	}
	lua_pushinteger(L, lz->zipFile->files[index - 1]->szU);
	return 1;
}

static int l_zip_OpenFile(lua_State* L)
{
	lzip_s* lz = GetZip(L, "OpenFile", true);
	int n = lua_gettop(L);
	if (n < 1 || !(lua_isnumber(L, 1) || lua_isstring(L, 1))) {
		luaL_error(L, "Usage: zip:OpenFile(index) or zip:OpenFile(fileName)");
	}
	fs_fileInfo_s* i = NULL;
	if (lua_isnumber(L, 1)) {
		int index = lua_tointeger(L, 1);
		if (index < 1 || index > lz->zipFile->numFiles) {
			luaL_error(L, "zip:OpenFile(): invalid index");
		}
		i = lz->zipFile->files[index - 1];
	} else {
		const char* name = lua_tostring(L, 1);
		for (int index = 0; index < lz->zipFile->numFiles; index++) {
			if ( !strcmp(name, lz->zipFile->files[index]->name) ) {
				i = lz->zipFile->files[index];
				break;
			}
		}
		if ( !i ) {
			return 0;
		}
	}
	fs_file_c* file = new fs_file_c(i);
	lzipFile_s* lzf = (lzipFile_s*)lua_newuserdata(L, sizeof(lzipFile_s));
	lzf->file = file;
	lua_pushvalue(L, lua_upvalueindex(2));
	lua_setmetatable(L, -2);
	return 1;
}

static int l_zipFile_Close(lua_State* L)
{
	lzipFile_s* lzf = GetZipFile(L, "Close", false);
	delete lzf->file;
	lzf->file = NULL;
	return 0;
}

static int l_zipFile_Read(lua_State* L)
{
	lzipFile_s* lzf = GetZipFile(L, "Read", true);
	int n = lua_gettop(L);
	if (n < 1) {
		luaL_error(L, "Usage: zipFile:Read(count) or zipFile:read(\"*a\")");
	}
	int count = 0;
	if (lua_isnumber(L, 1)) {
		count = lua_tointeger(L, 1);
	} else {
		if ( !strcmp(lua_tostring(L, 1), "*a") ) {
			count = lzf->file->Length() - lzf->file->Tell();
		} else {
			luaL_error(L, "zipFile:Read(): unrecognised format: %s", lua_tostring(L, 1));
		}
	}
	if (count < 1) {
		lua_pushstring(L, "");
		return 1;
	}
	byte* buf = new byte[count];
	lzf->file->Read(buf, count);
	lua_pushlstring(L, (char*)buf, count);
	return 1;
}

static int l_zipFile_Length(lua_State* L)
{
	lzipFile_s* lzf = GetZipFile(L, "Read", true);
	lua_pushinteger(L, lzf->file->Length());
	return 1;
}

extern "C" LZIP_EXPORT int luaopen_lzip(lua_State* L)
{
	lua_settop(L, 0);
	lua_newtable(L); // Library table
	lua_newtable(L); // Zip metatable
	lua_newtable(L); // Zip file metatable

	lua_pushvalue(L, 2);
	lua_setfield(L, 1, "zipMeta");
	lua_pushvalue(L, 2);
	lua_setfield(L, 2, "__index");

	lua_pushvalue(L, 3);
	lua_setfield(L, 1, "zipFileMeta");
	lua_pushvalue(L, 3);
	lua_setfield(L, 3, "__index");

	lua_pushvalue(L, 1);
	lua_pushvalue(L, 2);
	lua_pushcclosure(L, l_open, 2);
	lua_setfield(L, 1, "open");

	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zip_Close, 1);
	lua_setfield(L, 2, "__gc");
	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zip_Close, 1);
	lua_setfield(L, 2, "Close");
	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zip_GetNumFiles, 1);
	lua_setfield(L, 2, "GetNumFiles");
	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zip_GetFileName, 1);
	lua_setfield(L, 2, "GetFileName");
	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zip_GetFileSize, 1);
	lua_setfield(L, 2, "GetFileSize");
	lua_pushvalue(L, 1);
	lua_pushvalue(L, 3);
	lua_pushcclosure(L, l_zip_OpenFile, 2);
	lua_setfield(L, 2, "OpenFile");

	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zipFile_Close, 1);
	lua_setfield(L, 3, "__gc");
	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zipFile_Close, 1);
	lua_setfield(L, 3, "Close");
	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zipFile_Read, 1);
	lua_setfield(L, 3, "Read");
	lua_pushvalue(L, 1);
	lua_pushcclosure(L, l_zipFile_Length, 1);
	lua_setfield(L, 3, "Length");

	// Pop metatables
	lua_pop(L, 2);

	return 1;
}
