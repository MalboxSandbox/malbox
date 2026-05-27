/**
 * File type detection via magic byte signatures.
 *
 * Signature format and detection logic ported from CyberChef (Apache-2.0).
 * Original: https://github.com/gchq/CyberChef  (Crown Copyright 2018, n1474335)
 */

type SigValue = number | number[] | ((b: number) => boolean);
type Signature = Record<number, SigValue>;

export interface FileTypeMatch {
	name: string;
	extension: string;
	mime: string;
	description?: string;
}

interface FileSignatureEntry {
	name: string;
	extension: string;
	mime: string;
	description?: string;
	signature: Signature | Signature[];
}

function bytesMatch(sig: Signature, buf: Uint8Array, offset = 0): boolean {
	for (const sigoffset in sig) {
		const pos = parseInt(sigoffset, 10) + offset;
		if (pos >= buf.length) return false;
		const val = sig[sigoffset];
		switch (typeof val) {
			case 'number':
				if (buf[pos] !== val) return false;
				break;
			case 'object':
				if ((val as number[]).indexOf(buf[pos]) < 0) return false;
				break;
			case 'function':
				if (!val(buf[pos])) return false;
				break;
		}
	}
	return true;
}

function signatureMatches(sig: Signature | Signature[], buf: Uint8Array, offset = 0): boolean {
	if (Array.isArray(sig)) {
		for (let i = 0; i < sig.length; i++) {
			if (bytesMatch(sig[i], buf, offset)) return true;
		}
		return false;
	}
	return bytesMatch(sig, buf, offset);
}

const FILE_SIGNATURES: Record<string, FileSignatureEntry[]> = {
	Images: [
		{
			name: 'Joint Photographic Experts Group image',
			extension: 'jpg,jpeg',
			mime: 'image/jpeg',
			signature: {
				0: 0xff,
				1: 0xd8,
				2: 0xff,
				3: [
					0xc0, 0xc4, 0xdb, 0xdd, 0xe0, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe7, 0xe8, 0xea, 0xeb, 0xec,
					0xed, 0xee, 0xfe
				]
			}
		},
		{
			name: 'Graphics Interchange Format image',
			extension: 'gif',
			mime: 'image/gif',
			signature: { 0: 0x47, 1: 0x49, 2: 0x46, 3: 0x38, 4: [0x37, 0x39], 5: 0x61 }
		},
		{
			name: 'Portable Network Graphics image',
			extension: 'png',
			mime: 'image/png',
			signature: { 0: 0x89, 1: 0x50, 2: 0x4e, 3: 0x47, 4: 0x0d, 5: 0x0a, 6: 0x1a, 7: 0x0a }
		},
		{
			name: 'WEBP Image',
			extension: 'webp',
			mime: 'image/webp',
			signature: { 8: 0x57, 9: 0x45, 10: 0x42, 11: 0x50 }
		},
		{
			name: 'High Efficiency Image File Format',
			extension: 'heic,heif',
			mime: 'image/heif',
			signature: {
				0: 0x00,
				1: 0x00,
				2: 0x00,
				3: [0x24, 0x18],
				4: 0x66,
				5: 0x74,
				6: 0x79,
				7: 0x70,
				8: 0x68,
				9: 0x65,
				10: 0x69,
				11: 0x63
			}
		},
		{
			name: 'Camera Image File Format',
			extension: 'crw',
			mime: 'image/x-canon-crw',
			signature: {
				6: 0x48,
				7: 0x45,
				8: 0x41,
				9: 0x50,
				10: 0x43,
				11: 0x43,
				12: 0x44,
				13: 0x52
			}
		},
		{
			name: 'Canon CR2 raw image',
			extension: 'cr2',
			mime: 'image/x-canon-cr2',
			signature: [
				{ 0: 0x49, 1: 0x49, 2: 0x2a, 3: 0x0, 8: 0x43, 9: 0x52 },
				{ 0: 0x4d, 1: 0x4d, 2: 0x0, 3: 0x2a, 8: 0x43, 9: 0x52 }
			]
		},
		{
			name: 'Tagged Image File Format image',
			extension: 'tif',
			mime: 'image/tiff',
			signature: [
				{ 0: 0x49, 1: 0x49, 2: 0x2a, 3: 0x0 },
				{ 0: 0x4d, 1: 0x4d, 2: 0x0, 3: 0x2a }
			]
		},
		{
			name: 'Bitmap image',
			extension: 'bmp',
			mime: 'image/bmp',
			signature: {
				0: 0x42,
				1: 0x4d,
				7: 0x0,
				9: 0x0,
				14: [0x0c, 0x28, 0x38, 0x40, 0x6c, 0x7c],
				15: 0x0,
				16: 0x0,
				17: 0x0
			}
		},
		{
			name: 'JPEG Extended Range image',
			extension: 'jxr',
			mime: 'image/vnd.ms-photo',
			signature: { 0: 0x49, 1: 0x49, 2: 0xbc }
		},
		{
			name: 'Photoshop image',
			extension: 'psd',
			mime: 'image/vnd.adobe.photoshop',
			signature: {
				0: 0x38,
				1: 0x42,
				2: 0x50,
				3: 0x53,
				4: 0x0,
				5: 0x1,
				6: 0x0,
				7: 0x0,
				8: 0x0,
				9: 0x0,
				10: 0x0,
				11: 0x0
			}
		},
		{
			name: 'Icon image',
			extension: 'ico',
			mime: 'image/x-icon',
			signature: {
				0: 0x0,
				1: 0x0,
				2: 0x1,
				3: 0x0,
				4: [
					0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0x10, 0x11,
					0x12, 0x13, 0x14, 0x15
				],
				5: 0x0,
				6: [0x10, 0x20, 0x30, 0x40, 0x80],
				7: [0x10, 0x20, 0x30, 0x40, 0x80],
				9: 0x0,
				10: [0x0, 0x1]
			}
		},
		{
			name: 'Radiance High Dynamic Range image',
			extension: 'hdr',
			mime: 'image/vnd.radiance',
			signature: {
				0: 0x23,
				1: 0x3f,
				2: 0x52,
				3: 0x41,
				4: 0x44,
				5: 0x49,
				6: 0x41,
				7: 0x4e,
				8: 0x43,
				9: 0x45,
				10: 0x0a
			}
		},
		{
			name: 'AutoCAD Drawing',
			extension: 'dwg',
			mime: 'application/acad',
			signature: {
				0: 0x41,
				1: 0x43,
				2: 0x31,
				3: 0x30,
				4: [0x30, 0x31],
				5: [0x30, 0x31, 0x32, 0x33, 0x34, 0x35],
				6: 0x00
			}
		}
	],
	Video: [
		{
			name: 'Matroska Multimedia Container',
			extension: 'mkv',
			mime: 'video/x-matroska',
			signature: {
				31: 0x6d,
				32: 0x61,
				33: 0x74,
				34: 0x72,
				35: 0x6f,
				36: 0x73,
				37: 0x6b,
				38: 0x61
			}
		},
		{
			name: 'WEBM video',
			extension: 'webm',
			mime: 'video/webm',
			signature: { 0: 0x1a, 1: 0x45, 2: 0xdf, 3: 0xa3 }
		},
		{
			name: 'Flash MP4 video',
			extension: 'f4v',
			mime: 'video/mp4',
			signature: {
				4: 0x66,
				5: 0x74,
				6: 0x79,
				7: 0x70,
				8: [0x66, 0x46],
				9: 0x34,
				10: [0x76, 0x56],
				11: 0x20
			}
		},
		{
			name: 'MPEG-4 video',
			extension: 'mp4',
			mime: 'video/mp4',
			signature: [
				{ 0: 0x0, 1: 0x0, 2: 0x0, 3: [0x18, 0x20], 4: 0x66, 5: 0x74, 6: 0x79, 7: 0x70 },
				{ 0: 0x33, 1: 0x67, 2: 0x70, 3: 0x35 },
				{
					0: 0x0,
					1: 0x0,
					2: 0x0,
					3: 0x1c,
					4: 0x66,
					5: 0x74,
					6: 0x79,
					7: 0x70,
					8: 0x6d,
					9: 0x70,
					10: 0x34,
					11: 0x32
				}
			]
		},
		{
			name: 'M4V video',
			extension: 'm4v',
			mime: 'video/x-m4v',
			signature: {
				0: 0x0,
				1: 0x0,
				2: 0x0,
				3: 0x1c,
				4: 0x66,
				5: 0x74,
				6: 0x79,
				7: 0x70,
				8: 0x4d,
				9: 0x34,
				10: 0x56
			}
		},
		{
			name: 'Quicktime video',
			extension: 'mov',
			mime: 'video/quicktime',
			signature: { 0: 0x0, 1: 0x0, 2: 0x0, 3: 0x14, 4: 0x66, 5: 0x74, 6: 0x79, 7: 0x70 }
		},
		{
			name: 'Audio Video Interleave',
			extension: 'avi',
			mime: 'video/x-msvideo',
			signature: { 0: 0x52, 1: 0x49, 2: 0x46, 3: 0x46, 8: 0x41, 9: 0x56, 10: 0x49 }
		},
		{
			name: 'Windows Media Video',
			extension: 'wmv',
			mime: 'video/x-ms-wmv',
			signature: {
				0: 0x30,
				1: 0x26,
				2: 0xb2,
				3: 0x75,
				4: 0x8e,
				5: 0x66,
				6: 0xcf,
				7: 0x11,
				8: 0xa6,
				9: 0xd9
			}
		},
		{
			name: 'MPEG video',
			extension: 'mpg',
			mime: 'video/mpeg',
			signature: { 0: 0x0, 1: 0x0, 2: 0x1, 3: 0xba }
		},
		{
			name: 'Flash Video',
			extension: 'flv',
			mime: 'video/x-flv',
			signature: { 0: 0x46, 1: 0x4c, 2: 0x56, 3: 0x1 }
		}
	],
	Audio: [
		{
			name: 'Waveform Audio',
			extension: 'wav',
			mime: 'audio/x-wav',
			signature: { 0: 0x52, 1: 0x49, 2: 0x46, 3: 0x46, 8: 0x57, 9: 0x41, 10: 0x56, 11: 0x45 }
		},
		{
			name: 'OGG audio',
			extension: 'ogg',
			mime: 'audio/ogg',
			signature: { 0: 0x4f, 1: 0x67, 2: 0x67, 3: 0x53 }
		},
		{
			name: 'MIDI audio',
			extension: 'midi',
			mime: 'audio/midi',
			signature: { 0: 0x4d, 1: 0x54, 2: 0x68, 3: 0x64 }
		},
		{
			name: 'MPEG-3 audio',
			extension: 'mp3',
			mime: 'audio/mpeg',
			signature: [
				{ 0: 0x49, 1: 0x44, 2: 0x33 },
				{ 0: 0xff, 1: 0xfb }
			]
		},
		{
			name: 'MPEG-4 Part 14 audio',
			extension: 'm4a',
			mime: 'audio/m4a',
			signature: [
				{ 4: 0x66, 5: 0x74, 6: 0x79, 7: 0x70, 8: 0x4d, 9: 0x34, 10: 0x41 },
				{ 0: 0x4d, 1: 0x34, 2: 0x41, 3: 0x20 }
			]
		},
		{
			name: 'Free Lossless Audio Codec',
			extension: 'flac',
			mime: 'audio/x-flac',
			signature: { 0: 0x66, 1: 0x4c, 2: 0x61, 3: 0x43 }
		},
		{
			name: 'Adaptive Multi-Rate audio',
			extension: 'amr',
			mime: 'audio/amr',
			signature: { 0: 0x23, 1: 0x21, 2: 0x41, 3: 0x4d, 4: 0x52, 5: 0x0a }
		},
		{
			name: 'Audio Interchange File',
			extension: 'aif',
			mime: 'audio/x-aiff',
			signature: {
				0: 0x46,
				1: 0x4f,
				2: 0x52,
				3: 0x4d,
				8: 0x41,
				9: 0x49,
				10: 0x46,
				11: 0x46
			}
		}
	],
	Documents: [
		{
			name: 'Portable Document Format',
			extension: 'pdf',
			mime: 'application/pdf',
			signature: { 0: 0x25, 1: 0x50, 2: 0x44, 3: 0x46 }
		},
		{
			name: 'Rich Text Format',
			extension: 'rtf',
			mime: 'application/rtf',
			signature: { 0: 0x7b, 1: 0x5c, 2: 0x72, 3: 0x74 }
		},
		{
			name: 'Microsoft Office document/OLE2',
			extension: 'doc,xls,ppt',
			mime: 'application/msword',
			signature: {
				0: 0xd0,
				1: 0xcf,
				2: 0x11,
				3: 0xe0,
				4: 0xa1,
				5: 0xb1,
				6: 0x1a,
				7: 0xe1
			}
		},
		{
			name: 'Microsoft Office 2007+ document',
			extension: 'docx,xlsx,pptx',
			mime: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
			signature: {
				38: 0x5f,
				39: 0x54,
				40: 0x79,
				41: 0x70,
				42: 0x65,
				43: 0x73,
				44: 0x5d,
				45: 0x2e,
				46: 0x78,
				47: 0x6d,
				48: 0x6c
			}
		},
		{
			name: 'EPUB e-book',
			extension: 'epub',
			mime: 'application/epub+zip',
			signature: {
				0: 0x50,
				1: 0x4b,
				2: 0x3,
				3: 0x4,
				30: 0x6d,
				31: 0x69,
				32: 0x6d,
				33: 0x65,
				34: 0x74,
				35: 0x79,
				36: 0x70,
				37: 0x65,
				38: 0x61,
				39: 0x70,
				40: 0x70,
				41: 0x6c,
				42: 0x69,
				43: 0x63,
				44: 0x61,
				45: 0x74,
				46: 0x69,
				47: 0x6f,
				48: 0x6e,
				49: 0x2f,
				50: 0x65,
				51: 0x70,
				52: 0x75,
				53: 0x62,
				54: 0x2b,
				55: 0x7a,
				56: 0x69,
				57: 0x70
			}
		},
		{
			name: 'Microsoft OneNote document',
			extension: 'one',
			mime: 'application/onenote',
			signature: {
				0: 0xe4,
				1: 0x52,
				2: 0x5c,
				3: 0x7b,
				4: 0x8c,
				5: 0xd8,
				6: 0xa7,
				7: 0x4d,
				8: 0xae,
				9: 0xb1,
				10: 0x53,
				11: 0x78,
				12: 0xd0,
				13: 0x29,
				14: 0x96,
				15: 0xd3
			}
		},
		{
			name: 'PostScript',
			extension: 'ps',
			mime: 'application/postscript',
			signature: { 0: 0x25, 1: 0x21 }
		}
	],
	Applications: [
		{
			name: 'Windows Portable Executable',
			extension: 'exe,dll',
			mime: 'application/vnd.microsoft.portable-executable',
			signature: { 0: 0x4d, 1: 0x5a, 3: [0x0, 0x1, 0x2], 5: [0x0, 0x1, 0x2] }
		},
		{
			name: 'Executable and Linkable Format',
			extension: 'elf',
			mime: 'application/x-executable',
			signature: { 0: 0x7f, 1: 0x45, 2: 0x4c, 3: 0x46 }
		},
		{
			name: 'MacOS Mach-O object',
			extension: 'dylib',
			mime: 'application/octet-stream',
			signature: [
				{
					0: 0xca,
					1: 0xfe,
					2: 0xba,
					3: 0xbe,
					4: 0x00,
					5: 0x00,
					6: 0x00,
					7: [0x01, 0x02, 0x03]
				},
				{
					0: 0xce,
					1: 0xfa,
					2: 0xed,
					3: 0xfe,
					4: 0x07,
					5: 0x00,
					6: 0x00,
					7: 0x00,
					8: [0x01, 0x02, 0x03]
				}
			]
		},
		{
			name: 'MacOS Mach-O 64-bit object',
			extension: 'dylib',
			mime: 'application/octet-stream',
			signature: { 0: 0xcf, 1: 0xfa, 2: 0xed, 3: 0xfe }
		},
		{
			name: 'Adobe Flash',
			extension: 'swf',
			mime: 'application/x-shockwave-flash',
			signature: { 0: [0x43, 0x46], 1: 0x57, 2: 0x53 }
		},
		{
			name: 'Java Class',
			extension: 'class',
			mime: 'application/java-vm',
			signature: { 0: 0xca, 1: 0xfe, 2: 0xba, 3: 0xbe }
		},
		{
			name: 'Dalvik Executable',
			extension: 'dex',
			mime: 'application/octet-stream',
			signature: { 0: 0x64, 1: 0x65, 2: 0x78, 3: 0x0a, 4: 0x30, 5: 0x33, 6: 0x35, 7: 0x0 }
		},
		{
			name: 'Google Chrome Extension',
			extension: 'crx',
			mime: 'application/crx',
			signature: { 0: 0x43, 1: 0x72, 2: 0x32, 3: 0x34 }
		}
	],
	Archives: [
		{
			name: 'PKZIP archive',
			extension: 'zip',
			mime: 'application/zip',
			signature: { 0: 0x50, 1: 0x4b, 2: [0x3, 0x5, 0x7], 3: [0x4, 0x6, 0x8] }
		},
		{
			name: 'TAR archive',
			extension: 'tar',
			mime: 'application/x-tar',
			signature: { 257: 0x75, 258: 0x73, 259: 0x74, 260: 0x61, 261: 0x72 }
		},
		{
			name: 'Roshal Archive',
			extension: 'rar',
			mime: 'application/x-rar-compressed',
			signature: { 0: 0x52, 1: 0x61, 2: 0x72, 3: 0x21, 4: 0x1a, 5: 0x7, 6: [0x0, 0x1] }
		},
		{
			name: 'Gzip',
			extension: 'gz',
			mime: 'application/gzip',
			signature: { 0: 0x1f, 1: 0x8b, 2: 0x8 }
		},
		{
			name: 'Bzip2',
			extension: 'bz2',
			mime: 'application/x-bzip2',
			signature: { 0: 0x42, 1: 0x5a, 2: 0x68 }
		},
		{
			name: '7zip',
			extension: '7z',
			mime: 'application/x-7z-compressed',
			signature: { 0: 0x37, 1: 0x7a, 2: 0xbc, 3: 0xaf, 4: 0x27, 5: 0x1c }
		},
		{
			name: 'Zlib Deflate',
			extension: 'zlib',
			mime: 'application/x-deflate',
			signature: { 0: 0x78, 1: [0x1, 0x9c, 0xda, 0x5e] }
		},
		{
			name: 'xz compression',
			extension: 'xz',
			mime: 'application/x-xz',
			signature: { 0: 0xfd, 1: 0x37, 2: 0x7a, 3: 0x58, 4: 0x5a, 5: 0x0 }
		},
		{
			name: 'Tarball',
			extension: 'tar.z',
			mime: 'application/x-gtar',
			signature: { 0: 0x1f, 1: [0x9d, 0xa0] }
		},
		{
			name: 'Virtual Machine Disk',
			extension: 'vmdk',
			mime: 'application/vmdk',
			signature: { 0: 0x4b, 1: 0x44, 2: 0x4d, 3: 0x56, 5: 0x00, 6: 0x00, 7: 0x00 }
		},
		{
			name: 'Virtual Hard Drive',
			extension: 'vhd',
			mime: 'application/x-vhd',
			signature: { 0: 0x63, 1: 0x6f, 2: 0x6e, 3: 0x65, 4: 0x63, 5: 0x74, 6: 0x69, 7: 0x78 }
		},
		{
			name: 'ARJ Archive',
			extension: 'arj',
			mime: 'application/x-arj-compressed',
			signature: { 0: 0x60, 1: 0xea, 8: [0x0, 0x10, 0x14], 9: 0x0, 10: 0x2 }
		},
		{
			name: 'Microsoft Cabinet',
			extension: 'cab',
			mime: 'vnd.ms-cab-compressed',
			signature: { 0: 0x4d, 1: 0x53, 2: 0x43, 3: 0x46, 4: 0x00, 5: 0x00, 6: 0x00, 7: 0x00 }
		},
		{
			name: 'lzop compressed',
			extension: 'lzo',
			mime: 'application/x-lzop',
			signature: { 0: 0x89, 1: 0x4c, 2: 0x5a, 3: 0x4f, 4: 0x00, 5: 0x0d, 6: 0x0a, 7: 0x1a }
		},
		{
			name: 'Linux deb package',
			extension: 'deb',
			mime: 'application/vnd.debian.binary-package',
			signature: { 0: 0x21, 1: 0x3c, 2: 0x61, 3: 0x72, 4: 0x63, 5: 0x68, 6: 0x3e }
		},
		{
			name: 'ISO disk image',
			extension: 'iso',
			mime: 'application/octet-stream',
			description: 'ISO 9660 CD/DVD image file',
			signature: [
				{ 0x8001: 0x43, 0x8002: 0x44, 0x8003: 0x30, 0x8004: 0x30, 0x8005: 0x31 },
				{ 0x8801: 0x43, 0x8802: 0x44, 0x8803: 0x30, 0x8804: 0x30, 0x8805: 0x31 },
				{ 0x9001: 0x43, 0x9002: 0x44, 0x9003: 0x30, 0x9004: 0x30, 0x9005: 0x31 }
			]
		}
	],
	Miscellaneous: [
		{
			name: 'SQLite',
			extension: 'sqlite',
			mime: 'application/x-sqlite3',
			signature: { 0: 0x53, 1: 0x51, 2: 0x4c, 3: 0x69 }
		},
		{
			name: 'WinNT Registry Hive',
			extension: 'registry',
			mime: 'application/octet-stream',
			signature: { 0: 0x72, 1: 0x65, 2: 0x67, 3: 0x66 }
		},
		{
			name: 'Windows Event Log',
			extension: 'evt',
			mime: 'application/octet-stream',
			signature: { 0: 0x30, 1: 0x00, 2: 0x00, 3: 0x00, 4: 0x4c, 5: 0x66, 6: 0x4c, 7: 0x65 }
		},
		{
			name: 'Windows Event Log',
			extension: 'evtx',
			mime: 'application/octet-stream',
			signature: { 0: 0x45, 1: 0x6c, 2: 0x66, 3: 0x46, 4: 0x69, 5: 0x6c, 6: 0x65 }
		},
		{
			name: 'Windows Pagedump',
			extension: 'dmp',
			mime: 'application/octet-stream',
			signature: {
				0: 0x50,
				1: 0x41,
				2: 0x47,
				3: 0x45,
				4: 0x44,
				5: 0x55,
				6: [0x4d, 0x36],
				7: [0x50, 0x34]
			}
		},
		{
			name: 'Windows Prefetch',
			extension: 'pf',
			mime: 'application/x-pf',
			signature: {
				0: [0x11, 0x17, 0x1a],
				1: 0x0,
				2: 0x0,
				3: 0x0,
				4: 0x53,
				5: 0x43,
				6: 0x43,
				7: 0x41
			}
		},
		{
			name: 'PList (binary)',
			extension: 'bplist',
			mime: 'application/x-plist',
			signature: {
				0: 0x62,
				1: 0x70,
				2: 0x6c,
				3: 0x69,
				4: 0x73,
				5: 0x74,
				6: 0x30,
				7: 0x30
			}
		},
		{
			name: 'Windows Shortcut',
			extension: 'lnk',
			mime: 'application/x-ms-shortcut',
			signature: {
				0: 0x4c,
				1: 0x00,
				2: 0x00,
				3: 0x00,
				4: 0x01,
				5: 0x14,
				6: 0x02,
				7: 0x00,
				8: 0x00,
				9: 0x00,
				10: 0x00,
				11: 0x00,
				12: 0xc0,
				13: 0x00,
				14: 0x00,
				15: 0x00,
				16: 0x00,
				17: 0x00,
				18: 0x00,
				19: 0x46
			}
		},
		{
			name: 'PCAP',
			extension: 'pcap',
			mime: 'application/vnd.tcpdump.pcap',
			signature: [
				{ 0: 0xd4, 1: 0xc3, 2: 0xb2, 3: 0xa1 },
				{ 0: 0xa1, 1: 0xb2, 2: 0xc3, 3: 0xd4 }
			]
		},
		{
			name: 'PCAPng',
			extension: 'pcapng',
			mime: 'application/octet-stream',
			signature: { 0: 0x0a, 1: 0x0d, 2: 0x0d, 3: 0x0a }
		},
		{
			name: 'Compiled HTML',
			extension: 'chm',
			mime: 'application/vnd.ms-htmlhelp',
			signature: { 0: 0x49, 1: 0x54, 2: 0x53, 3: 0x46, 4: 0x03, 5: 0x00, 6: 0x00, 7: 0x00 }
		},
		{
			name: 'Web Open Font Format',
			extension: 'woff',
			mime: 'application/font-woff',
			signature: { 0: 0x77, 1: 0x4f, 2: 0x46, 3: 0x46, 4: 0x0, 5: 0x1, 6: 0x0, 7: 0x0 }
		},
		{
			name: 'Web Open Font Format 2',
			extension: 'woff2',
			mime: 'application/font-woff',
			signature: { 0: 0x77, 1: 0x4f, 2: 0x46, 3: 0x32, 4: 0x0, 5: 0x1, 6: 0x0, 7: 0x0 }
		},
		{
			name: 'TrueType Font',
			extension: 'ttf',
			mime: 'application/font-sfnt',
			signature: { 0: 0x0, 1: 0x1, 2: 0x0, 3: 0x0, 4: 0x0 }
		},
		{
			name: 'OpenType Font',
			extension: 'otf',
			mime: 'application/font-sfnt',
			signature: { 0: 0x4f, 1: 0x54, 2: 0x54, 3: 0x4f, 4: 0x0 }
		},
		{
			name: 'Lua Bytecode',
			extension: 'luac',
			mime: 'application/x-lua',
			signature: { 0: 0x1b, 1: 0x4c, 2: 0x75, 3: 0x61 }
		},
		{
			name: 'WebAssembly binary',
			extension: 'wasm',
			mime: 'application/wasm',
			signature: { 0: 0x00, 1: 0x61, 2: 0x73, 3: 0x6d }
		},
		{
			name: 'Certificate',
			extension: 'cer,der',
			mime: 'application/pkix-cert',
			signature: { 0: 0x30, 1: 0x82, 4: [0x06, 0x0a, 0x30] }
		},
		{
			name: 'php',
			extension: 'php',
			mime: 'application/php',
			signature: { 0: 0x3c, 1: 0x3f, 2: 0x70, 3: 0x68, 4: 0x70 }
		}
	]
};

export function detectFileType(
	buf: Uint8Array,
	categories: string[] = Object.keys(FILE_SIGNATURES)
): FileTypeMatch[] {
	if (!buf || buf.length < 2) return [];

	const matching: FileTypeMatch[] = [];

	for (const cat of categories) {
		const entries = FILE_SIGNATURES[cat];
		if (!entries) continue;
		for (const entry of entries) {
			if (signatureMatches(entry.signature, buf)) {
				matching.push({
					name: entry.name,
					extension: entry.extension,
					mime: entry.mime,
					description: entry.description
				});
			}
		}
	}

	return matching;
}

const TRANSFORM_EXTENSIONS: Record<string, string> = {
	gzip: 'gz',
	deflate: 'deflate',
	'zlib-compress': 'zlib',
	'base64-encode': 'txt',
	'base32-encode': 'txt',
	'hex-encode': 'txt',
	'hex-dump': 'txt',
	'url-encode': 'txt',
	strings: 'txt',
	entropy: 'txt',
	'magic-bytes': 'txt',
	'frequency-analysis': 'json',
	md5: 'txt',
	sha1: 'txt',
	sha256: 'txt',
	sha512: 'txt',
	'all-hashes': 'txt',
	'xor-bruteforce': 'txt',
	'xor-multikey-detect': 'txt',
	'deobfuscate-strings': 'txt',
	'regex-extract': 'txt'
};

function isTextData(data: Uint8Array): boolean {
	if (data.length === 0) return true;
	try {
		new TextDecoder('utf-8', { fatal: true }).decode(data);
		return true;
	} catch {
		return false;
	}
}

export function detectFileExtension(data: Uint8Array, lastTransformId?: string): string {
	const types = detectFileType(data);
	if (types.length) {
		return types[0].extension.split(',')[0];
	}

	if (lastTransformId && lastTransformId in TRANSFORM_EXTENSIONS) {
		return TRANSFORM_EXTENSIONS[lastTransformId];
	}

	if (isTextData(data)) return 'txt';

	return 'bin';
}
