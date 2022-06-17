using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using CommunityToolkit.Mvvm.Input;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Moudle;
using Microsoft.Win32;
using PropertyChanged;
using TagLib;
using static Fanhuaji_API.Fanhuaji;

namespace ConvertZZ.ViewModels
{
    [AddINotifyPropertyChangedInterface]
    public class AudioTagsPageVIewModel
    {
        public AudioTagsPageVIewModel()
        {
            ConvertCommand = new AsyncRelayCommand(async () =>
            {
                CommandButtonIsEnable = false;
                Stopwatch stopwatch = new();
                stopwatch.Start();

                var temp = FileList.Where(x => x.IsChecked).ToList();
                string? ErrorMessage = null;
                foreach (var _temp in temp)
                {
                    Mouse.OverrideCursor = Cursors.Wait;
                    stopwatch.Start();
                    try
                    {
                        FileStream fileStream = new(Path.Combine(_temp.Path, _temp.Name), FileMode.Open, FileAccess.ReadWrite);
                        var tfile = TagLib.File.Create(new StreamFileAbstraction(Path.Combine(_temp.Path, _temp.Name), fileStream, fileStream));
                        tfile.RemoveTags((Enable_ID3v1 ? TagLib.TagTypes.None : TagLib.TagTypes.Id3v1) | (Enable_ID3v2 ? TagLib.TagTypes.None : TagLib.TagTypes.Id3v2));
                        TagLib.Id3v1.Tag t = (TagLib.Id3v1.Tag)tfile.GetTag(TagLib.TagTypes.Id3v1, Enable_ID3v1 ? true : false);
                        TagLib.Id3v2.Tag t2 = (TagLib.Id3v2.Tag)tfile.GetTag(TagLib.TagTypes.Id3v2, Enable_ID3v2 ? true : false);
                        SetID3v2Encoding(Encoding_Output_ID3v2);
                        if (t != null)
                        {
                            var TagList = GetAllStringProperties(t);
                            var Dic = TagList.ToDictionary(x => x.TagName, x => StringToUnicode.TryToConvertLatin1ToUnicode(x.Value, encoding[0]));

                            if (ConvertEncoding)
                                Dic = await App.Instance.ConvertEncodingAndTextAsync(Dic, encoding[0], encoding[1], ToChinese1);
                            else
                                Dic = await App.Instance.ConvertEncodingAndTextAsync(Dic, Encoding.UTF8, Encoding.UTF8, ToChinese1);

                            Dic.ToList().ForEach(x => t.SetPropertiesValue(x.Key, Encoding.GetEncoding("ISO-8859-1").GetString(encoding[1].GetBytes(x.Value))));
                        }
                        if (t2 != null)
                        {
                            var TagList = GetAllStringProperties(t2);
                            var Dic = TagList.ToDictionary(x => x.TagName, x =>
                            {
                                if (tfile.TagTypesOnDisk.HasFlag(TagLib.TagTypes.Id3v2))
                                    return StringToUnicode.TryToConvertLatin1ToUnicode(x.Value, encoding2[0]);
                                else
                                {
                                    var _ = ID3v1_TagList.Where(y => y.TagName == x.TagName).FirstOrDefault();
                                    return _ != null ? _.Value_Preview : "";
                                }
                            });

                            Dic = await App.Instance.ConvertEncodingAndTextAsync(Dic, Encoding.UTF8, Encoding.UTF8, ToChinese2);
                            Dic.ToList().ForEach(x => t.SetPropertiesValue(x.Key, x.Value));

                            t2.Version = (Combobox_ID3v2_VersionText == "2.3") ? (byte)3 : (byte)4;
                        }
                        tfile.Save();
                    }
                    catch (TagLib.UnsupportedFormatException) { ErrorMessage = string.Format("轉換{0}時出現錯誤，該檔案並非音訊檔", _temp.Name); }
                    catch (FanhuajiException val)
                    {
                        ErrorMessage = ((Exception)val).Message;
                        break;
                    }
                    catch { ErrorMessage = string.Format("轉換{0}時出現未知錯誤", _temp.Name); }
                }
                Mouse.OverrideCursor = null;
                stopwatch.Stop();
                if (!string.IsNullOrEmpty(ErrorMessage))
                    Window_MessageBoxEx.ShowDialog(ErrorMessage, "轉換過程中出現錯誤", "我知道了");
                else if (App.Settings.Prompt)
                {
                    new Toast(string.Format("轉換完成\r\n耗時：{0} ms", stopwatch.ElapsedMilliseconds)).Show();
                }
                CommandButtonIsEnable = true;
                Listview_SelectionChanged();
            });
            ClearCommand = new RelayCommand(() =>
            {
                FileList.Clear();
                ID3v1_TagList.Clear();
                ID3v2_TagList.Clear();
                LastPath = "";
            });
            SelectFileCommand = new RelayCommand(() =>
            {
                OpenFileDialog fileDialog = new() { Multiselect = true, CheckFileExists = false, CheckPathExists = true, ValidateNames = false };
                fileDialog.InitialDirectory = App.Settings.FileConvert.DefaultPath;
                fileDialog.FileName = "　";
                fileDialog.Filter = SelectedFilter;
                if (fileDialog.ShowDialog() == true)
                {
                    ImportFileNames(fileDialog.FileNames);
                }
            });
            FilterEditorCommand = new RelayCommand(() =>
            {
                App.Settings.FileConvert.CallFilterEditor();
                FilterList = new(App.Settings.FileConvert.GetFilterList());
            });
        }

        public enum Format
        {
            ID3,
            APE,
            OGG
        }

        /// <summary>
        /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
        /// </summary>
        private Encoding[] encoding = new Encoding[2] { Encoding.GetEncoding("GBK"), Encoding.GetEncoding("Big5") };

        /// <summary>
        /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
        /// </summary>
        private Encoding[] encoding2 = new Encoding[2] { Encoding.GetEncoding("GBK"), Encoding.GetEncoding("Big5") };

        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        [OnChangedMethod(nameof(Preview))]
        public ETextConvertMode ToChinese1 { get; set; } = ETextConvertMode.None;

        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        public ETextConvertMode ToChinese2 { get; set; } = ETextConvertMode.None;

        private bool ConvertEncoding = true;
        private string LastPath = "";

        private void SetID3v2Encoding(Encoding Encoding)
        {
            TagLib.Id3v2.Tag.ForceDefaultEncoding = true;
            switch (Encoding.EncodingName)
            {
                case "ISO-8859-1":
                    TagLib.Id3v2.Tag.DefaultEncoding = TagLib.StringType.Latin1;
                    break;

                case "UTF-8":
                    TagLib.Id3v2.Tag.DefaultEncoding = TagLib.StringType.UTF8;
                    break;

                case "Unicode":
                case "UTF-16":
                    TagLib.Id3v2.Tag.DefaultEncoding = TagLib.StringType.UTF16;
                    break;

                case "UTF-16BE":
                    TagLib.Id3v2.Tag.DefaultEncoding = TagLib.StringType.UTF16BE;
                    break;

                case "UTF-16LE":
                    TagLib.Id3v2.Tag.DefaultEncoding = TagLib.StringType.UTF16LE;
                    break;

                default:
                    {
                    }
                    break;
            }
        }

        private List<TagList_Line> GetAllStringProperties(object obj)
        {
            List<TagList_Line> keyValuePair = new();
            foreach (PropertyDescriptor descriptor in TypeDescriptor.GetProperties(obj))
            {
                string name = descriptor.Name;
                object value = descriptor.GetValue(obj);

                dynamic vars = value as string;
                if (vars != null)
                {
                    keyValuePair.Add(new TagList_Line() { IsChecked = true, TagName = name, Value = value as string });
                }
            }
            return keyValuePair;
        }

        private void Listview_SelectionChanged()
        {
            if (SelectedItem is not null)
            {
                string path = Path.Combine(SelectedItem.Path, SelectedItem.Name);
                if (System.IO.File.Exists(path))
                {
                    LastPath = path;
                    Preview(path);
                }
            }
        }

        private void ApplyFilter()
        {
            ICollectionView collectionView = CollectionViewSource.GetDefaultView(FileList);
            collectionView.Filter = t =>
            {
                if (SelectedFilter is null)
                    return true;
                if (t is FileList_Line line)
                    foreach (var x in App.Settings.FileConvert.GetExtentionArray(SelectedFilter.ToString()))
                    {
                        if (App.Settings.FileConvert.CheckExtension(line.Name, x))
                            return true;
                    }
                return false;
            };
        }

        public void ImportFileNames(string[] FileNames)
        {
            string ParentPath = Path.GetDirectoryName(FileNames.First());
            foreach (string str in FileNames)
            {
                if ((Path.GetFileNameWithoutExtension(str) == "　" || Directory.Exists(str)) && System.IO.Directory.Exists(System.IO.Path.GetDirectoryName(str)))
                {
                    string folderpath = System.IO.Path.GetDirectoryName(str);
                    App.Settings.FileConvert.GetExtentionArray(SelectedFilter).ForEach(filter =>
                    {
                        List<string> childFileList = System.IO.Directory.GetFiles(folderpath, filter.Trim(), AccordingToChild ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly).Where(x => App.Settings.FileConvert.CheckExtension(x, filter)).ToList();
                        childFileList.ForEach(x =>
                        {
                            FileList.Add(new FileList_Line() { IsChecked = true, IsFile = true, Name = System.IO.Path.GetFileName(x), ParentPath = ParentPath, Path = Path.GetDirectoryName(x) });
                        });
                    });
                    FileList = new ObservableCollection<FileList_Line>(FileList.OrderBy(x => x.Name).Distinct().OrderBy(x => x.IsFile).OrderBy(x => x.Path));
                }
                else if (System.IO.File.Exists(str))
                {
                    FileList.Add(new FileList_Line() { IsChecked = true, Name = Path.GetFileName(str), ParentPath = ParentPath, Path = Path.GetDirectoryName(str) });
                }
            }
        }

        private void Preview()
        {
            Preview(LastPath);
        }

        private async void Preview(string path)
        {
            if (!System.IO.File.Exists(path))
                return;
            try
            {
                FileStream fileStream = new(path, FileMode.Open, FileAccess.Read);
                var tfile = TagLib.File.Create(new StreamFileAbstraction(path, fileStream, fileStream), TagLib.ReadStyle.None);
                TagLib.Id3v1.Tag t = (TagLib.Id3v1.Tag)tfile.GetTag(TagLib.TagTypes.Id3v1);
                TagLib.Id3v2.Tag t2 = (TagLib.Id3v2.Tag)tfile.GetTag(TagLib.TagTypes.Id3v2);

                ID3v1_TagList.Clear();
                ID3v2_TagList.Clear();

                var TagList = GetAllStringProperties(t);
                TagList.ForEach(x => x.Value = StringToUnicode.TryToConvertLatin1ToUnicode(x.Value, encoding[0]));
                var Dic = TagList.ToDictionary(x => x.TagName, x => x.Value);
                if (ConvertEncoding)
                    Dic = await App.Instance.ConvertEncodingAndTextAsync(Dic, encoding[0], encoding[1], ToChinese1);
                else
                    Dic = await App.Instance.ConvertEncodingAndTextAsync(Dic, Encoding.UTF8, Encoding.UTF8, ToChinese1);

                TagList.ForEach(x =>
                {
                    x.Value_Preview = Dic[x.TagName];
                    ID3v1_TagList.Add(x);
                });

                TagList = GetAllStringProperties(t2);
                Dic = TagList.ToDictionary(x => x.TagName, x =>
                {
                    if (tfile.TagTypesOnDisk.HasFlag(TagLib.TagTypes.Id3v2))
                        return StringToUnicode.TryToConvertLatin1ToUnicode(x.Value, encoding2[0]);
                    else
                    {
                        var _ = ID3v1_TagList.Where(y => y.TagName == x.TagName).FirstOrDefault();
                        return _ != null ? _.Value_Preview : "";
                    }
                });

                Dic = await App.Instance.ConvertEncodingAndTextAsync(Dic, Encoding.UTF8, Encoding.UTF8, ToChinese2);
                TagList.ForEach(x =>
                {
                    x.Value_Preview = Dic[x.TagName];
                    ID3v2_TagList.Add(x);
                });
            }
            catch (TagLib.UnsupportedFormatException)
            {
                ID3v1_TagList.Clear();
                ID3v2_TagList.Clear();
                ID3v1_TagList.Add(new TagList_Line() { TagName = "Error", Value = "非音訊檔" });
                ID3v2_TagList.Add(new TagList_Line() { TagName = "Error", Value = "非音訊檔" });
            }
            catch (FanhuajiException val)
            {
                ID3v1_TagList.Clear();
                ID3v2_TagList.Clear();
                ID3v1_TagList.Add(new TagList_Line
                {
                    TagName = "Error",
                    Value = val.Message
                });
                ID3v2_TagList.Add(new TagList_Line
                {
                    TagName = "Error",
                    Value = val.Message
                });
            }
            catch (System.Exception)
            {
                ID3v1_TagList.Clear();
                ID3v2_TagList.Clear();
                ID3v1_TagList.Add(new TagList_Line() { TagName = "Error", Value = "未知" });
                ID3v2_TagList.Add(new TagList_Line() { TagName = "Error", Value = "未知" });
            }
        }

        public bool ID3Mode { get; set; } = true;

        public bool AccordingToChild { get; set; } = true;

        public ObservableCollection<FileList_Line> FileList { get; set; } = new ObservableCollection<FileList_Line>();

        public ObservableCollection<TagList_Line> ID3v1_TagList { get; set; } = new ObservableCollection<TagList_Line>();

        public ObservableCollection<Encoding> Encoding_Input { get; set; } = new ObservableCollection<Encoding>(new Encoding[] { Encoding.GetEncoding("BIG5"), Encoding.GetEncoding("GBK"), Encoding.GetEncoding("Shift-JIS") });
        public ObservableCollection<Encoding> Encoding_Output { get; set; } = new ObservableCollection<Encoding>(new Encoding[] { Encoding.GetEncoding("BIG5"), Encoding.GetEncoding("GBK"), Encoding.GetEncoding("Shift-JIS") });
        public ObservableCollection<TagList_Line> ID3v2_TagList { get; set; }
        public Encoding Encoding_Source_ID3v1 { get; set; }
        public Encoding Encoding_Output_ID3v1 { get; set; }
        public Encoding Encoding_Source_ID3v2 { get; set; }
        public Encoding Encoding_Output_ID3v2 { get; set; } = Encoding.GetEncoding("UTF-16");

        public bool Enable_ID3v1 { get; set; } = true;
        public bool Enable_ID3v2 { get; set; } = true;

        public bool CommandButtonIsEnable { get; set; }
        public string? Combobox_ID3v2_VersionText { get; set; }
        public ComboBoxItem? Combobox_ID3v2_Version { get; set; }

        [OnChangedMethod(nameof(ApplyFilter))]
        public string? SelectedFilter { get; set; }

        public ObservableCollection<string> FilterList { get; set; }

        public ICommand ClearCommand { get; }
        public ICommand ConvertCommand { get; }
        public ICommand SelectFileCommand { get; }
        public ICommand FilterEditorCommand { get; }

        [OnChangedMethod(nameof(Listview_SelectionChanged))]
        public FileList_Line SelectedItem { get; set; }

        public class FileList_Line
        {
            public int ID { get; set; }
            public bool IsChecked { get; set; }     //or IsSelected maybe? whichever name you want
            public bool IsFile { get; set; }
            public string Name { get; set; }
            public string ParentPath { get; set; }
            public string Path { get; set; }
        }

        public class TagList_Line
        {
            public bool IsChecked { get; set; }     //or IsSelected maybe? whichever name you want
            public string TagName { get; set; }
            public string Value { get; set; }
            public string Value_Preview { get; set; }
        }
    }
}