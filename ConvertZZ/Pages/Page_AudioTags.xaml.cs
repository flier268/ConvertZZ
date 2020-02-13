using ConvertZZ.Moudle;
using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using static Fanhuaji_API.Fanhuaji;

namespace ConvertZZ.Pages
{
    /// <summary>
    /// Page_AudioTags.xaml 的互動邏輯
    /// </summary>
    public partial class Page_AudioTags : Page, INotifyPropertyChanged
    {
        public Page_AudioTags(Format format)
        {
            InitializeComponent();
            DataContext = this;
            ComboBox_ID3v2_Version_SelectionChanged(Combobox_Encoding_ID3v2, null);
            Combobox_Filter.ItemsSource = App.Settings.FileConvert.GetFilterList();
            Combobox_Filter.SelectedIndex = 0;
        }
        public Page_AudioTags(Format format, string[] FileNames) : this(format)
        {
            if (FileNames == null)
                return;
            ImportFileNames(FileNames);
            Combobox_Filter_SelectionChanged(Combobox_Filter, null);
        }
        private void Page_Loaded(object sender, RoutedEventArgs e)
        {
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
        Encoding[] encoding = new Encoding[2] { Encoding.GetEncoding("GBK"), Encoding.GetEncoding("Big5") };
        /// <summary>
        /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
        /// </summary>
        Encoding[] encoding2 = new Encoding[2] { Encoding.GetEncoding("GBK"), Encoding.GetEncoding("Big5") };
        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        int ToChinese1 = 0;
        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        int ToChinese2 = 0;
        private bool ConvertEncoding = true;

        string LastPath = "";
        private async void Button_Convert_Click(object sender, RoutedEventArgs e)
        {
            ((Button)e.Source).IsEnabled = false;
            Stopwatch stopwatch = new Stopwatch();
            stopwatch.Start();

            var temp = FileList.Where(x => x.IsChecked).ToList();
            string ErrorMessage = null;
            foreach (var _temp in temp)
            {
                Mouse.OverrideCursor = Cursors.Wait;
                stopwatch.Start();
                try
                {
                    var tfile = TagLib.File.Create(Path.Combine(_temp.Path, _temp.Name));
                    tfile.RemoveTags((Enable_ID3v1 ? TagLib.TagTypes.None : TagLib.TagTypes.Id3v1) | (Enable_ID3v2 ? TagLib.TagTypes.None : TagLib.TagTypes.Id3v2));
                    TagLib.Id3v1.Tag t = (TagLib.Id3v1.Tag)tfile.GetTag(TagLib.TagTypes.Id3v1, Enable_ID3v1 ? true : false);
                    TagLib.Id3v2.Tag t2 = (TagLib.Id3v2.Tag)tfile.GetTag(TagLib.TagTypes.Id3v2, Enable_ID3v2 ? true : false);
                    SetID3v2Encoding(Encoding_Output_ID3v2);
                    if (t != null)
                    {
                        var TagList = GetAllStringProperties(t);
                        var Dic = TagList.ToDictionary(x => x.TagName, x => StringToUnicode.TryToConvertLatin1ToUnicode(x.Value, encoding[0]));
                        var resoult = ConvertEncoding ? await ConvertHelper.ConvertDictionary(Dic, encoding, ToChinese1) : await ConvertHelper.ConvertDictionary(Dic, ToChinese1);
                        resoult.ToList().ForEach(x => t.SetPropertiesValue(x.Key, Encoding.GetEncoding("ISO-8859-1").GetString(encoding[1].GetBytes(x.Value))));
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
                        var resoult = await ConvertHelper.ConvertDictionary(Dic, ToChinese2);
                        resoult.ToList().ForEach(x => t.SetPropertiesValue(x.Key, x.Value));
                        t2.Version = (Combobox_ID3v2_Version.Text == "2.3") ? (byte)3 : (byte)4;
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
            ((Button)e.Source).IsEnabled = true;
            Listview_SelectionChanged(null, null);
        }
        private async void Preview(string path)
        {
            if (!File.Exists(path))
                return;
            try
            {
                var tfile = TagLib.File.Create(path, TagLib.ReadStyle.None);
                TagLib.Id3v1.Tag t = (TagLib.Id3v1.Tag)tfile.GetTag(TagLib.TagTypes.Id3v1);
                TagLib.Id3v2.Tag t2 = (TagLib.Id3v2.Tag)tfile.GetTag(TagLib.TagTypes.Id3v2);

                ID3v1_TagList.Clear();
                ID3v2_TagList.Clear();

                var TagList = GetAllStringProperties(t);
                TagList.ForEach(x => x.Value = StringToUnicode.TryToConvertLatin1ToUnicode(x.Value, encoding[0]));
                var Dic = TagList.ToDictionary(x => x.TagName, x => x.Value);
                var resoult = ConvertEncoding ? await ConvertHelper.ConvertDictionary(Dic, encoding, ToChinese1) : await ConvertHelper.ConvertDictionary(Dic, ToChinese1);
                TagList.ForEach(x =>
                {
                    x.Value_Preview = resoult[x.TagName];
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
                resoult = await ConvertHelper.ConvertDictionary(Dic, ToChinese2);
                TagList.ForEach(x =>
                {
                    x.Value_Preview = resoult[x.TagName];
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
        private List<TagList_Line> GetAllStringProperties(object obj)
        {
            List<TagList_Line> keyValuePair = new List<TagList_Line>();
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
        private bool SetPropertiesValue(object obj, string key, object value)
        {
            obj.GetType().GetProperty(key).SetValue(obj, value, null);
            return true;
            /*
            List<TagList_Line> keyValuePair = new List<TagList_Line>();
            foreach (PropertyDescriptor descriptor in TypeDescriptor.GetProperties(obj))
            {
                if (descriptor.Name == key)
                {
                    descriptor.SetValue(obj, value);
                    return true;
                }
            }
            return false;
            */
        }
        private void ModeChange(object sender, RoutedEventArgs e)
        {

        }
        private void Listview_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (listview.SelectedItem != null)
            {
                FileList_Line line = ((FileList_Line)listview.SelectedItem);
                string path = Path.Combine(line.Path, line.Name);
                if (File.Exists(path))
                {
                    LastPath = path;
                    Preview(path);
                }
            }
        }
        private void Button_SelectFile_Clicked(object sender, RoutedEventArgs e)
        {
            OpenFileDialog fileDialog = new OpenFileDialog() { Multiselect = true, CheckFileExists = false, CheckPathExists = true, ValidateNames = false };
            fileDialog.InitialDirectory = App.Settings.FileConvert.DefaultPath;
            fileDialog.FileName = "　";
            fileDialog.Filter = Combobox_Filter.SelectedValue.ToString();
            if (fileDialog.ShowDialog() == true)
            {
                ImportFileNames(fileDialog.FileNames);
                Combobox_Filter_SelectionChanged(Combobox_Filter, null);
            }
        }
        private void ImportFileNames(string[] FileNames)
        {
            string ParentPath = Path.GetDirectoryName(FileNames.First());
            foreach (string str in FileNames)
            {
                if ((Path.GetFileNameWithoutExtension(str) == "　" || Directory.Exists(str)) && System.IO.Directory.Exists(System.IO.Path.GetDirectoryName(str)))
                {
                    string folderpath = System.IO.Path.GetDirectoryName(str);
                    App.Settings.FileConvert.GetExtentionArray(Combobox_Filter.Text).ForEach(filter =>
                    {
                        List<string> childFileList = System.IO.Directory.GetFiles(folderpath, filter.Trim(), AccordingToChild ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly).Where(x => App.Settings.FileConvert.CheckExtension(x, filter)).ToList();
                        childFileList.ForEach(x =>
                        {
                            FileListTemp.Add(new FileList_Line() { IsChecked = true, IsFile = true, Name = System.IO.Path.GetFileName(x), ParentPath = ParentPath, Path = Path.GetDirectoryName(x) });
                        });
                    });
                    FileListTemp = new ObservableCollection<FileList_Line>(FileListTemp.OrderBy(x => x.Name).Distinct().OrderBy(x => x.IsFile).OrderBy(x => x.Path));
                }
                else if (File.Exists(str))
                {
                    FileListTemp.Add(new FileList_Line() { IsChecked = true, Name = Path.GetFileName(str), ParentPath = ParentPath, Path = Path.GetDirectoryName(str) });
                }
            }
        }
        private void Button_Clear_Clicked(object sender, RoutedEventArgs e)
        {
            FileList.Clear();
            FileListTemp.Clear();
            ID3v1_TagList.Clear();
            ID3v2_TagList.Clear();
            LastPath = "";
        }
        private void Encoding_Selected(object sender, RoutedEventArgs e)
        {
            RadioButton radiobutton = ((RadioButton)sender);
            switch (radiobutton.GroupName)
            {
                case "origin":
                    encoding[0] = Encoding.GetEncoding(((string)radiobutton.Content).Trim());
                    break;
                case "target":
                    encoding[1] = Encoding.GetEncoding(((string)radiobutton.Content).Trim());
                    break;
            }
            ModeChange(null, null);
        }
        private void Chinese_Click(object sender, RoutedEventArgs e)
        {
            switch (((RadioButton)sender).Uid)
            {
                case "NChinese1":
                    ToChinese1 = 0;
                    break;
                case "TChinese1":
                    ToChinese1 = 1;
                    break;
                case "CChinese1":
                    ToChinese1 = 2;
                    break;
                case "NChinese2":
                    ToChinese2 = 0;
                    break;
                case "TChinese2":
                    ToChinese2 = 1;
                    break;
                case "CChinese2":
                    ToChinese2 = 2;
                    break;
            }
            Preview(LastPath);
        }

        private void SetID3v2Encoding(string Encoding)
        {
            TagLib.Id3v2.Tag.ForceDefaultEncoding = true;
            switch (Encoding)
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
            }
        }

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

        public bool ID3Mode { get; set; } = true;

        public bool AccordingToChild { get; set; } = true;

        public ObservableCollection<FileList_Line> FileList { get; set; } = new ObservableCollection<FileList_Line>();

        public ObservableCollection<TagList_Line> ID3v1_TagList { get; set; } = new ObservableCollection<TagList_Line>();
        private ObservableCollection<TagList_Line> _ID3v2_TagList = new ObservableCollection<TagList_Line>();
        public ObservableCollection<TagList_Line> ID3v2_TagList { get => _ID3v2_TagList; set { _ID3v2_TagList = value; } }

        public bool Enable_ID3v1 { get; set; } = true;
        public bool Enable_ID3v2 { get; set; } = true;
        private string _Encoding_Source_ID3v1 = "GBK";
        private string _Encoding_Output_ID3v1 = "Big5";
        private string _Encoding_Source_ID3v2 = "GBK";
        public string Encoding_Source_ID3v1 { get => _Encoding_Source_ID3v1; set { _Encoding_Source_ID3v1 = value; encoding[0] = Encoding.GetEncoding(value); Preview(LastPath); } }
        public string Encoding_Output_ID3v1
        {
            get => _Encoding_Output_ID3v1;
            set
            {
                _Encoding_Output_ID3v1 = value;
                var temp = value.Split(new char[] { '(' });
                ConvertEncoding = temp.Length == 1;
                encoding[1] = Encoding.GetEncoding(temp[0]);
                Preview(LastPath);
            }
        }
        public string Encoding_Source_ID3v2 { get => _Encoding_Source_ID3v2; set { _Encoding_Source_ID3v2 = value; encoding2[0] = Encoding.GetEncoding(value); Preview(LastPath); } }
        public string Encoding_Output_ID3v2 { get; set; } = "UTF-16";

        public event PropertyChangedEventHandler PropertyChanged;



        private void ComboBox_ID3v2_Version_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (Combobox_Encoding_ID3v2 == null)
                return;
            string[] Version3 = { "UTF-16", "UTF-16LE", "UTF-16BE" };
            string[] Version4 = { "UTF-8", "UTF-16", "UTF-16LE", "UTF-16BE" };
            ComboBox comboBox = (sender as ComboBox);
            if ((ComboBoxItem)comboBox.SelectedItem == null)
                Combobox_Encoding_ID3v2.ItemsSource = Version3;
            else
            {
                switch (((ComboBoxItem)comboBox.SelectedItem).Content)
                {
                    case "2.3":
                        Combobox_Encoding_ID3v2.ItemsSource = Version3;
                        break;
                    case "2.4":
                        Combobox_Encoding_ID3v2.ItemsSource = Version4;
                        break;
                }
            }
            if (Combobox_Encoding_ID3v2.SelectedValue == null)
                Combobox_Encoding_ID3v2.SelectedItem = "UTF-16";
        }
        ObservableCollection<FileList_Line> FileListTemp = new ObservableCollection<FileList_Line>();
        private void Combobox_Filter_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            ObservableCollection<FileList_Line> temp = new ObservableCollection<FileList_Line>();
            if ((sender as ComboBox).SelectedValue == null)
            {
                FileList = FileListTemp;
                (sender as ComboBox).SelectedIndex = 0;
                return;
            }
            App.Settings.FileConvert.GetExtentionArray((sender as ComboBox).SelectedValue.ToString()).ForEach(x =>
            {
                foreach (var t in FileListTemp)
                    if (App.Settings.FileConvert.CheckExtension(t.Name, x))
                        temp.Add(t);
            });
            FileList = new ObservableCollection<FileList_Line>(temp.Distinct());
        }

        private void Button_FilterEditor_Click(object sender, RoutedEventArgs e)
        {
            App.Settings.FileConvert.CallFilterEditor();
            Combobox_Filter.ItemsSource = App.Settings.FileConvert.GetFilterList();
        }
    }
}
