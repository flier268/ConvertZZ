using Microsoft.Win32;
using Microsoft.WindowsAPICodePack.Dialogs;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Text;
using System.Windows;
using System.Windows.Controls;

namespace ConvertZZ.Pages
{
    /// <summary>
    /// Page_File.xaml 的互動邏輯
    /// </summary>
    public partial class Page_File : Page, INotifyPropertyChanged
    {
        public Page_File()
        {
            DataContext = this;
            OutputPath = App.Settings.FileConvert.DefaultPath.Substring(App.Settings.FileConvert.DefaultPath[0] == '!' ? 1 : 0);            
            InitializeComponent();
        }
        /// <summary>
         /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
         /// </summary>
        Encoding[] encoding = new Encoding[2] { Encoding.GetEncoding("BIG5"), Encoding.GetEncoding("GBK") };
        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        int ToChinese = 0;
        private string Convert(string origin)
        {
            switch (ToChinese)
            {
                case 1:
                    origin = ChineseConverter.ToTraditional(origin);
                    if (App.Settings.VocabularyCorrection)
                        origin = App.ChineseConverter.Convert(origin);
                    break;
                case 2:
                    origin = ChineseConverter.ToSimplified(origin);
                    if (App.Settings.VocabularyCorrection)
                        origin = App.ChineseConverter.Convert(origin);
                    break;
            }
            return encoding[1].GetString(encoding[0].GetBytes(origin));
        }
        private void Button_Convert_Click(object sender, RoutedEventArgs e)
        {
            switch (FileMode)
            {
                case true:
                    {
                        var temp = FileList.Where(x => x.isChecked).ToList();
                        bool replaceALL = false;
                        bool skip = false;
                        foreach (var _temp in temp)
                        {
                            if (!replaceALL && File.Exists(Path.Combine(OutputPath, _temp.Name)))
                            {
                                if (!skip)
                                    switch (Moudle.Window_MessageBoxEx.Show(string.Format("{0}發生檔名衝突，是否取代?", _temp.Name), "警告", "取代", "略過", "取消", "套用到全部"))
                                    {
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.A:
                                            break;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.B:
                                            continue;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.C:
                                            return;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.AO:
                                            replaceALL = true;
                                            break;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.BO:
                                            skip = true;
                                            continue;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.CO:
                                            return;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.NONE:
                                            continue;
                                    }
                                else
                                    continue;
                            }
                            string str = "";
                            using (StreamReader sr = new StreamReader(Path.Combine(_temp.Path, _temp.Name), encoding[0]))
                            {
                                str = sr.ReadToEnd();
                                sr.Close();
                            }
                            str = Convert(str);
                            using (StreamWriter sw = new StreamWriter(Path.Combine(OutputPath, _temp.Name), false, encoding[1]))
                            {
                                sw.Write(str);
                                sw.Flush();
                            }
                        }
                    }
                    break;
                case false:
                    {
                        var temp = FileList.Where(x => x.isChecked).ToList();
                        foreach (var _temp in temp)
                        {
                            try
                            {
                                string _Path = Path.Combine(_temp.Path, _temp.Name);
                                if (File.Exists(_Path))
                                {
                                    string newName = Convert(_temp.Name);
                                    if (newName != _temp.Name)
                                        File.Move(_Path, Path.Combine(_temp.Path, newName));
                                }
                                else if (Directory.Exists(_Path))
                                {
                                    string newName = Convert(Path.GetFileName(_temp.Path));
                                    if (newName != Path.GetFileName(_temp.Path))
                                        Directory.Move(_temp.Path, Path.Combine(System.IO.Path.GetDirectoryName(_temp.Path), newName));
                                }
                            }
                            catch { }
                        }
                    }
                    break;
            }
        }
        private void Button_Clear_Clicked(object sender, RoutedEventArgs e)
        {
            FileList.Clear();
        }
        private void Button_SelectFile_Clicked(object sender, RoutedEventArgs e)
        {
            OpenFileDialog fileDialog = new OpenFileDialog() { Multiselect = true, CheckFileExists = false, CheckPathExists = true, ValidateNames = false };
            fileDialog.InitialDirectory = App.Settings.FileConvert.DefaultPath;
            fileDialog.FileName = "　";
            if (fileDialog.ShowDialog() == true)
            {
                OutputPath = Path.GetDirectoryName(fileDialog.FileNames.First());
                foreach (string str in fileDialog.FileNames)
                {
                    if (Path.GetFileName(str) == "　" && System.IO.Directory.Exists(System.IO.Path.GetDirectoryName(str)))
                    {
                        string folderpath = System.IO.Path.GetDirectoryName(str);
                        List<string> childFileList = System.IO.Directory.GetFiles(folderpath, "*", AccordingToChild ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly).ToList();
                        childFileList.ForEach(x =>
                        {
                            FileList.Add(new FileList_Line() { isChecked = true, isFile = true, Name = System.IO.Path.GetFileName(x), Path = Path.GetDirectoryName(x) });
                        });
                        if (!FileMode)
                        {
                            if (AccordingToMomFolder)
                                FileList.Add(new FileList_Line() { Name = Path.GetFileName(folderpath), isFile = false, Path = Path.GetDirectoryName(folderpath), isChecked = true });
                            List<string> childDirectoriesList = System.IO.Directory.GetDirectories(folderpath, "*", AccordingToChild ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly).ToList();
                            childDirectoriesList.ForEach(x =>
                            {
                                FileList.Add(new FileList_Line() { isChecked = true, isFile = false, Name = x.Substring(folderpath.Length + 1), Path = folderpath });
                            });
                        }
                        FileList = new ObservableCollection<FileList_Line>(FileList.OrderBy(x => x.Name).OrderBy(x => x.isFile).OrderBy(x => x.Path));
                    }
                    else if (File.Exists(str))
                    {
                        FileList.Add(new FileList_Line() { isChecked = true, Name = Path.GetFileName(str), Path = Path.GetDirectoryName(str) });
                    }
                }
                //listview.ItemsSource = FileList;
                if(!FileMode)
                {
                    StringBuilder sb = new StringBuilder();
                    FileList.ToList().ForEach(x => sb.AppendLine(x.Name));
                    InputPreviewText = sb.ToString();
                    OutputPreviewText = Convert(InputPreviewText);
                }
            }
        }


        private string _ClipBoard = "";
        public string ClipBoard { get => _ClipBoard; set { _ClipBoard = value; OnPropertyChanged("ClipBoard"); } }
        private string _InputPreviewText = "";
        public string InputPreviewText { get => _InputPreviewText; set { _InputPreviewText = value; OnPropertyChanged("InputPreviewText"); } }
        private string _OutputPreviewText = "";
        public string OutputPreviewText { get => _OutputPreviewText; set { _OutputPreviewText = value; OnPropertyChanged("OutputPreviewText"); } }
        private bool _FileMode = true;
        public bool FileMode { get => _FileMode; set { _FileMode = value; OnPropertyChanged("FileMode"); } }
        private bool _AccordingToChild = true;
        public bool AccordingToChild { get => _AccordingToChild; set { _AccordingToChild = value; OnPropertyChanged("AccordingToChild"); } }
        private bool _AccordingToMomFolder = true;
        public bool AccordingToMomFolder { get => _AccordingToMomFolder; set { _AccordingToMomFolder = value; OnPropertyChanged("AccordingToMomFolder"); } }
        private string _OutputPath = "";
        public string OutputPath { get => _OutputPath; set { _OutputPath = value; OnPropertyChanged("OutputPath"); } }


        private ObservableCollection<FileList_Line> _FileList = new ObservableCollection<FileList_Line>();

        public ObservableCollection<FileList_Line> FileList { get => _FileList; set { _FileList = value; OnPropertyChanged("FileList"); } }

        public class FileList_Line
        {
            public int ID { get; set; }
            public bool isChecked { get; set; }     //or IsSelected maybe? whichever name you want  
            public bool isFile { get; set; }
            public string Name { get; set; }
            public string Path { get; set; }
        }
        public event PropertyChangedEventHandler PropertyChanged;

        protected void OnPropertyChanged(string name) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));


        private void Encoding_Selected(object sender, RoutedEventArgs e)
        {
            RadioButton radiobutton = ((RadioButton)sender);
            switch (radiobutton.GroupName)
            {
                case "origin":
                    encoding[0] = Encoding.GetEncoding((string)radiobutton.Content);
                    break;
                case "target":
                    encoding[1] = Encoding.GetEncoding((string)radiobutton.Content);
                    break;
            }
            listview_SelectionChanged(null, null);
        }
        private void listview_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if ( FileMode)
            {
                if (listview.SelectedItem != null)
                {
                    FileList_Line line = ((FileList_Line)listview.SelectedItem);
                    string path = Path.Combine(line.Path, line.Name);
                    if (File.Exists(path))
                    {
                        using (StreamReader sr = new StreamReader(path, encoding[0]))
                        {
                            char[] c = null;
                            if (sr.Peek() >= 0)
                            {
                                c = new char[App.Settings.MaxLengthPreview];
                                sr.Read(c, 0, c.Length);
                                InputPreviewText = new string(c);
                            }
                        }
                        OutputPreviewText = Convert(InputPreviewText);
                    }
                }
            }
            else
            {
                StringBuilder sb = new StringBuilder();
                FileList.ToList().ForEach(x => sb.AppendLine(x.Name));
                InputPreviewText = sb.ToString();
                OutputPreviewText = Convert(InputPreviewText);
            }
        }
        private void Chinese_Click(object sender, RoutedEventArgs e)
        {
            switch (((RadioButton)sender).Uid)
            {
                case "NChinese":
                    ToChinese = 0;
                    break;
                case "TChinese":
                    ToChinese = 1;
                    break;
                case "CChinese":
                    ToChinese = 2;
                    break;
            }
            listview_SelectionChanged(null, null);
        }

        private void ModeChange(object sender, RoutedEventArgs e) => listview_SelectionChanged(null, null);

        private void Button_OutputPath_Click(object sender, RoutedEventArgs e)
        {
            var dlg = new CommonOpenFileDialog();
            dlg.Title = "Select Output folder";
            dlg.IsFolderPicker = true;
            dlg.AddToMostRecentlyUsedList = false;
            dlg.AllowNonFileSystemItems = false;
            dlg.EnsureFileExists = true;
            dlg.EnsurePathExists = true;
            dlg.EnsureReadOnly = false;
            dlg.EnsureValidNames = true;
            dlg.Multiselect = false;
            dlg.ShowPlacesList = true;

            if (dlg.ShowDialog() == CommonFileDialogResult.Ok)
            {
                OutputPath = dlg.FileName;
            }
        }
    }
}
