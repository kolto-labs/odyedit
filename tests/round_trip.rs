//! Round trips over real instruction files.
//!
//! The corpus is every distinct `changes.ini` the patcher's own test suite
//! builds. Between them they cover all eight sections and the awkward corners
//! the patcher had to be taught about: duplicate keys, source-folder
//! directives, inline dialog edits, archive destinations. Whatever the editor
//! does to one of these has to leave a file the patcher still reads the same
//! way.

use odyedit::ini::IniFile;
use odyedit::model::{gff, install, script, settings, ssf, tlk, twoda};

/// Instruction files taken from the patcher's test suite.
const CORPUS: &[&[u8]] = &[
    b"[Settings]\nInstallerMode=1\nFileExists=1\nLogLevel=4\n\n[TLKList]\nStrRef0=0\nStrRef1=1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nLogLevel=4\n\n[TLKList]\nReplace0=append.tlk\nStrRef0=0\nStrRef1=1\n\n[append.tlk]\n2=3\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nLogLevel=4\n\n[TLKList]\nAppend0=extra.tlk\n\n[extra.tlk]\n0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\n0\\Text=new text\n0\\Sound=vo_line\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nLogLevel=4\n\n[TLKList]\nStrRef0=0\n!SourceFile=append.tlk\n!SourceFileF=appendf.tlk\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\nReplace0=fixes.tlk\n1\\Text=inline both\n\n[fixes.tlk]\n0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\nAppend0=extra.tlk\n\n[extra.tlk]\n0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\n!DefaultSourceFolder=tlks\nReplace0=fixes.tlk\n\n[fixes.tlk]\n0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\n!DefaultSourceFolder=tlks\nStrRef0=0\nReplace0=fixes.tlk\n\n[fixes.tlk]\n0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\nStrRef0=0\n",
    b"[TLKList]\nStrRef0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\nStrRef0=0\nAppend0=extra.tlk\n\n[extra.tlk]\n0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\n!DefaultSourceFolder=tlks\nStrRef0=0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nBackupFiles=1\n\n[InstallList]\ninstall_override=override\n\n[install_override]\nFile0=newicon.tga\nReplace0=replacement.tga\nFile1=kept.tga\n",
    b"[InstallList]\nloose=\n\n[loose]\nFile0=newicon.tga\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\nChangeRow0=edit_first\nAddColumn0=add_flag\nCopyRow0=clone_second\n\n[new_spell]\nRowLabel=high()\nlabel=third_spell\nname=2000\niconresref=ip_third\n\n[edit_first]\nRowIndex=0\nname=9999\n\n[add_flag]\nColumnLabel=newflag\nDefaultValue=0\ni0=1\n\n[clone_second]\nRowIndex=1\nNewRowLabel=high()\nlabel=cloned_spell\n",
    b"[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nChangeRow0=edit\n\n[edit]\nRowIndex=0\nname=changed\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=from_desc\nChangeRow0=edit_first\nCopyRow0=clone_named\n\n[from_desc]\nRowLabel=high()\nlabel=third_spell\nname=high(spelldesc)\n\n[edit_first]\nRowIndex=0\nname=high(spelldesc)\n\n[clone_named]\nRowIndex=1\nNewRowLabel=high()\nlabel=cloned_spell\nname=high(spelldesc)\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\nStrRef0=0\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=capture_row\nChangeRow0=by_label_index\nChangeRow1=by_row_label_strref\nCopyRow0=from_memory\nCopyRow1=from_label_index\n\n[capture_row]\nlabel=captured_spell\nname=3000\n2DAMEMORY1=RowIndex\n2DAMEMORY2=label\n\n[by_label_index]\nLabelIndex=2DAMEMORY2\nname=3001\n\n[by_row_label_strref]\nRowLabel=StrRef0\nname=4000\n\n[from_memory]\nRowIndex=2DAMEMORY1\nNewRowLabel=from_mem\nlabel=copied_from_memory\n\n[from_label_index]\nLabelIndex=first_spell\nNewRowLabel=from_label\nlabel=copied_from_label\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=broken.2da\nTable1=spells.2da\n\n[broken.2da]\nAddRow0=never\n\n[never]\nlabel=should_not_exist\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=third_spell\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nCost=50\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\nTable1=broken.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=third_spell\n\n[broken.2da]\nAddRow0=never\n\n[never]\nlabel=should_not_exist\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=captured_spell\n2DAMEMORY1=RowIndex\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nCost=2DAMEMORY1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[TLKList]\nStrRef0=0\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=token_spell\nname=StrRef0\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nCost=500\nTag=new_tag\nAddField0=add_charges\n\n[add_charges]\nFieldType=Byte\nLabel=Charges\nValue=42\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nAddField0=add_property\n\n[add_property]\nFieldType=Struct\nPath=PropertiesList\nLabel=\nTypeId=0\n2DAMEMORY1=ListIndex\nAddField0=add_property_name\n\n[add_property_name]\nFieldType=Word\nLabel=PropertyName\nValue=45\n",
    b"[GFFList]\nFile0=item.uti\n\n[item.uti]\nAddField0=a\n\n[a]\nFieldType=Struct\nLabel=A\nAddField0=b\n\n[b]\nFieldType=Struct\nPath=A\nLabel=B\nAddField0=a\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[SSFList]\nFile0=sound.ssf\n\n[sound.ssf]\nBattlecry 1=100\nDeath=200\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[HACKList]\nFile0=patch.bin\n\n[patch.bin]\n4=258\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[HACKList]\nFile0=patch.ncs\n\n[patch.ncs]\n8=258\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[HACKList]\nFile0=patch.bin\nFile1=patch.ncs\n\n[patch.bin]\n4=u8:255\n8=u16:65535\n12=u32:1\n\n[patch.ncs]\n4=u32:1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=hacked_spell\n2DAMEMORY1=label\n2DAMEMORY2=RowIndex\n\n[HACKList]\nFile0=patch.bin\n\n[patch.bin]\n4=u16:2DAMEMORY2\n8=u32:0x01020304\n16=2DAMEMORY32:2\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[InstallList]\nloose=override\n\n[loose]\n!SourceFolder=textures\nFile0=icon.tga\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[InstallList]\nloose=override\n\n[loose]\n!SourceFolder=..\nFile0=icon.tga\n",
    b"[InstallList]\nescaped=./../outside\nsafe=override\n\n[escaped]\nFile0=escaped.tga\n\n[safe]\nFile0=safe.tga\n",
    b"[InstallList]\nloose=override\ncapsule=modules\\danm13.mod\n\n[loose]\nFile0=bad-folder\nFile1=safe.tga\n\n[bad-folder]\n!SourceFile=bad-folder.tga\n!SaveAs=bad-folder.tga\n!Destination=../outside\n\n[capsule]\nFile0=bad-archive\n\n[bad-archive]\n!SourceFile=bad-archive.tga\n!SaveAs=bad-archive.tga\n!Destination=..\\outside\n",
    b"[InstallList]\nloose=override\n\n[loose]\nFile0=bad-source\nFile1=bad-save\nFile2=safe.tga\n\n[bad-source]\n!SourceFile=../outside.tga\n!SaveAs=stolen.tga\n\n[bad-save]\n!SourceFile=inside.tga\n!SaveAs=../escaped.tga\n",
    b"[2DAList]\nTable0=unsafe\nTable1=safe\n\n[unsafe]\n!SourceFile=spells.2da\n!Filename=unsafe.2da\n!Destination=linked\nChangeRow0=edit_unsafe\n\n[edit_unsafe]\nRowIndex=0\nname=unsafe\n\n[safe]\n!SourceFile=spells.2da\n!Filename=safe.2da\nChangeRow0=edit_safe\n\n[edit_safe]\nRowIndex=0\nname=safe\n",
    b"[InstallList]\nloose=override\n\n[loose]\nReplace0=icon.tga\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nIgnoreExtensions=1\n\n[InstallList]\nloose=override\n\n[loose]\nFile0=foo.txi\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\n!Destination=modules\\danm13.mod\nCost=750\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\n!DefaultDestination=modules\\danm13.mod\nFile0=item.uti\n\n[item.uti]\nCost=750\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[InstallList]\ninto_module=modules\\danm13.mod\n\n[into_module]\nFile0=added.uti\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=add_existing\n\n[add_existing]\nExclusiveColumn=label\nlabel=first_spell\nname=7777\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nRequired=prereq.2da\nRequiredMsg=Install the other mod first.\n\n[2DAList]\nTable0=spells.2da\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nRequired=one.2da, two.2da\nRequiredMsg=Need both.\n\n[2DAList]\nTable0=spells.2da\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nRequired=prereq.2da\nRequiredMsg=Install the other mod first.\n\n[InstallList]\nloose=override\n\n[loose]\nFile0=icon.tga\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nBackupFiles=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nChangeRow0=edit\n\n[edit]\nRowIndex=0\nname=4242\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nBackupFiles=1\nLogLevel=4\n\n[TLKList]\nStrRef0=0\n\n[InstallList]\ninstall_override=override\n\n[install_override]\nFile0=loose.tga\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=combined_spell\nname=StrRef0\n2DAMEMORY1=RowIndex\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nCost=2DAMEMORY1\nTag=combined\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nCost=500\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=creature.utc\n\n[creature.utc]\nAddField0=add_inner\n\n[add_inner]\nFieldType=Int\nPath=Container\nLabel=Added\nValue=-7\n",
    b"[Settings]\nInstallerMode=1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nAddField0=bad_cost\n\n[bad_cost]\nFieldType=DWORD\nLabel=Cost\nValue=not_a_number\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[SSFList]\nFile0=sound.ssf\n\n[sound.ssf]\nBattlecry 1=100\nNot A Real Entry=200\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[InstallList]\ninto_module=modules\\danm13.mod\n\n[into_module]\nFile0=added.uti\n\n[added.uti]\n!OverrideType=rename\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nLogLevel=3\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nLogLevel=3\nPlaintextLog=1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nChangeRow0=edit\n\n[edit]\nRowIndex=2DAMEMORY5\nname=1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=thing.utp\n\n[thing.utp]\nWeight=1.234.567\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[CompileList]\nFile0=k_test.nss\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[CompileList]\n!DefaultDestination=modules\nFile0=k_test.nss\n",
    b"[CompileList]\nFile0=script\n\n[script]\n!SourceFile=../outside.nss\n!Filename=outside.nss\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[CompileList]\nFile0=helper.nss\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nChangeRow0=edit_first\n\n[edit_first]\nRowIndex=0\nname=9999\n\n[CompileList]\nFile0=k_test.nss\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=token_spell\n2DAMEMORY1=RowIndex\n\n[CompileList]\nFile0=k_mem.nss\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=add_existing\n\n[add_existing]\nExclusiveColumn=label\nlabel=first_spell\nRowLabel=high()\nname=7777\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=add_existing\n\n[add_existing]\nExclusiveColumn=label\nlabel=first_spell\n2DAMEMORY1=RowIndex\nname=7777\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nCost=2DAMEMORY1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nAddField0=change_cost\n\n[change_cost]\nFieldType=DWORD\nLabel=Cost\nValue=500\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[InstallList]\ninstall_override=override\n\n[install_override]\n!SourceFolder=scripts\nFile0=loose.tga\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nAddField0=add_bits\nAddField1=add_hex\nAddField2=add_b64\n\n[add_bits]\nFieldType=Binary\nLabel=BitsBlob\nValue=10101010\n\n[add_hex]\nFieldType=Binary\nLabel=HexBlob\nValue=0xDEAD\n\n[add_b64]\nFieldType=Binary\nLabel=B64Blob\nValue=QUJD\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\nFile1=other.uti\n\n[item.uti]\nAddField0=add_charges\n2DAMEMORY2=2DAMEMORY1\n\n[add_charges]\nFieldType=Byte\nLabel=Charges\nValue=1\n2DAMEMORY1=!FieldPath\n\n[other.uti]\n2DAMEMORY2=7\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=item.uti\nFile1=other.uti\n\n[item.uti]\nAddField0=add_charges\n2DAMEMORY1=!FieldPath\n\n[add_charges]\nFieldType=Byte\nLabel=Charges\nValue=1\n2DAMEMORY1=!FieldPath\n\n[other.uti]\n2DAMEMORY1=7\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nReplace0=item.uti\n\n[item.uti]\nCost=50\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=missing.uti\nFile1=item.uti\n\n[missing.uti]\nCost=1\n\n[item.uti]\nCost=50\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[GFFList]\nFile0=bad.uti\nFile1=item.uti\n\n[bad.uti]\nCost=1\n\n[item.uti]\nCost=50\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nChangeRow0=read_name\n\n[read_name]\nRowIndex=0\nStrRef5=name\n\n[GFFList]\nFile0=item.uti\n\n[item.uti]\nCost=StrRef5\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=rowindex\nname=rowlabel\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=spells.2da\n\n[spells.2da]\nChangeRow0=edit_first\nCopyRow0=clone_second\n\n[edit_first]\nRowIndex=0\nname=rowindex\n\n[clone_second]\nRowIndex=1\niconresref=rowindex\nspelldesc=rowlabel\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\n!DefaultDestination=modules\\danm13.mod\nTable0=spells.2da\n\n[spells.2da]\nAddRow0=new_spell\n\n[new_spell]\nlabel=archive_spell\n\n[HACKList]\nFile0=patch.tga\n\n[patch.tga]\n!Destination=modules\\danm13.mod\n4=258\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[InstallList]\nloose=override\n\n[loose]\nFile0=foo.tga\nFile1=kept.tga\n\n[foo.tga]\n!ReplaceFile=1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[InstallList]\nloose=override\n\n[loose]\nFile0=folder.tga\nFile1=added.uti\n\n[folder.tga]\n!Destination=modules\n\n[added.uti]\n!Destination=modules\\danm13.mod\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\nBackupFiles=1\n\n[InstallList]\nloose=override\n\n[loose]\nReplace0=icon.tga\n",
    b"[Settings]\nInstallerMode=0\nFileExists=1\n",
    b"[Settings]\nInstallerMode=1\nFileExists=1\n\n[2DAList]\nTable0=missing.2da\n",
];

/// The same files with Windows line endings, which is how they ship.
fn as_crlf(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(bytes)
        .replace('\n', "\r\n")
        .into_bytes()
}

#[test]
fn opening_and_saving_changes_nothing() {
    // This is the property the whole editor rests on. An operator who opens a
    // file, looks at it and saves it must get the same bytes back — otherwise
    // every session produces a diff nobody asked for.
    for (index, raw) in CORPUS.iter().enumerate() {
        let ini = IniFile::parse(raw);
        assert_eq!(
            ini.to_bytes(),
            raw.to_vec(),
            "file {index} changed when nothing was edited"
        );

        let crlf = as_crlf(raw);
        let ini = IniFile::parse(&crlf);
        assert_eq!(ini.to_bytes(), crlf, "file {index} lost its line endings");
    }
}

/// Read every section into its model and write every one back.
fn rewrite_everything(ini: &mut IniFile) {
    settings::Settings::read(ini).write(ini);

    let tokens = tlk::read(ini);
    tlk::write(ini, &tokens);

    let tables = twoda::read(ini);
    twoda::write(ini, &tables);

    let files = gff::read(ini);
    gff::write(ini, &files);

    let scripts = script::read(ini);
    script::write(ini, &scripts);

    let folders = install::read(ini);
    install::write(ini, &folders);

    let sounds = ssf::read(ini);
    ssf::write(ini, &sounds);
}

#[test]
fn writing_every_section_back_reaches_a_fixed_point() {
    // Panels write their whole section back whenever anything in it changes.
    // Doing that twice has to produce the same file the first pass did, or a
    // file would drift a little further every time it was touched.
    for (index, raw) in CORPUS.iter().enumerate() {
        let mut ini = IniFile::parse(raw);
        rewrite_everything(&mut ini);
        let once = ini.to_bytes();

        rewrite_everything(&mut ini);
        assert_eq!(
            ini.to_bytes(),
            once,
            "file {index} kept changing on a second write"
        );
    }
}

#[test]
fn a_write_never_loses_what_the_models_hold() {
    // Writing a section back and reading it again has to give the same answer.
    // Anything else means the editor is quietly dropping part of the file.
    for (index, raw) in CORPUS.iter().enumerate() {
        let mut ini = IniFile::parse(raw);

        let settings_before = settings::Settings::read(&ini);
        let tokens_before = tlk::read(&ini);
        let tables_before = twoda::read(&ini);
        let gff_before = gff::read(&ini);
        let scripts_before = script::read(&ini);
        let installs_before = install::read(&ini);
        let ssf_before = ssf::read(&ini);

        rewrite_everything(&mut ini);

        assert_eq!(
            settings::Settings::read(&ini),
            settings_before,
            "file {index} lost settings"
        );
        assert_eq!(tlk::read(&ini), tokens_before, "file {index} lost tokens");
        assert_eq!(twoda::read(&ini), tables_before, "file {index} lost tables");
        assert_eq!(gff::read(&ini), gff_before, "file {index} lost game files");
        assert_eq!(
            script::read(&ini),
            scripts_before,
            "file {index} lost scripts"
        );
        assert_eq!(
            install::read(&ini),
            installs_before,
            "file {index} lost install folders"
        );
        assert_eq!(ssf::read(&ini), ssf_before, "file {index} lost soundsets");
    }
}

#[test]
fn a_write_never_moves_a_key_it_does_not_own() {
    // The no-loss check compares what the models hold, which cannot see a key
    // the editor does not model sliding to a different place in its section.
    // Several sections mix dialects, and a key whose meaning is unknown is not
    // one this editor is in a position to reposition.
    for (index, raw) in CORPUS.iter().enumerate() {
        let mut ini = IniFile::parse(raw);
        let sections = ini.section_names();
        let before: Vec<Vec<String>> = sections.iter().map(|name| ini.read_section(name)).collect();

        rewrite_everything(&mut ini);

        for (name, keys) in sections.iter().zip(before) {
            // Modifier sections have two orderings the patcher forces; every
            // other section has to come back exactly as it went in.
            if name.eq_ignore_ascii_case("TLKList") {
                assert_eq!(
                    ini.read_section(name),
                    keys,
                    "file {index} reordered [{name}]"
                );
            }
        }
    }
}

#[test]
fn sections_the_editor_does_not_model_are_left_alone() {
    // HACKList has no panel yet. It still has to survive a session untouched.
    let raw =
        b"[Settings]\nInstallerMode=1\n\n[HACKList]\nFile0=patch.ncs\n\n[patch.ncs]\n0x10=5\n";
    let mut ini = IniFile::parse(raw);
    rewrite_everything(&mut ini);
    assert_eq!(ini.read_string("HACKList", "File0", ""), "patch.ncs");
    assert_eq!(ini.read_string("patch.ncs", "0x10", ""), "5");
}

#[test]
fn comments_survive_an_edit_elsewhere() {
    let raw = b"; keep this\n[Settings]\nLogLevel=4\n; and this\n\n[2DAList]\nTable0=spells.2da\n";
    let mut ini = IniFile::parse(raw);
    let mut values = settings::Settings::read(&ini);
    values.window_caption = "Renamed".to_string();
    values.write(&mut ini);

    let out = String::from_utf8(ini.to_bytes()).unwrap();
    assert!(out.contains("; keep this"));
    assert!(out.contains("; and this"));
    assert!(out.contains("WindowCaption=Renamed"));
    assert!(out.contains("Table0=spells.2da"));
}

#[test]
fn the_editor_opens_a_real_file_without_falling_over() {
    // Constructing the window's state is where every panel's buffer is filled
    // from the document. A file that makes one of them panic would take the
    // whole program down on open, which no round-trip check would catch.
    let dir = std::env::temp_dir().join("odyedit-open-test");
    std::fs::create_dir_all(&dir).unwrap();

    for (index, raw) in CORPUS.iter().enumerate() {
        let path = dir.join(format!("changes{index}.ini"));
        std::fs::write(&path, raw).unwrap();
        let app = odyedit::ui::App::with_file(path.to_str().unwrap());
        // The file opened, so it is the one the editor is holding.
        assert!(!app.is_dirty(), "file {index} looked edited on open");
    }

    std::fs::remove_dir_all(&dir).ok();
}
