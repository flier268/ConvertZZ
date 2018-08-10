using ConvertZZ.Moudle;
using Microsoft.Win32;
using Microsoft.WindowsAPICodePack.Dialogs;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Text.RegularExpressions;
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
            InitializeComponent();
            _UseFilter = App.Settings.FileConvert.UseFilter;
            DataContext = this;
            OutputPath = App.Settings.FileConvert.DefaultPath.Substring(App.Settings.FileConvert.DefaultPath[0] == '!' ? 1 : 0);
        }
        /// <summary>
        /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
        /// </summary>
        Encoding[] encoding = new Encoding[2] { Encoding.UTF8, Encoding.UTF8 };
        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        int ToChinese = 0;
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
                            string TargetPath = Path.Combine(Path.Combine(OutputPath, _temp.Path.Substring(_temp.ParentPath.Length + (_temp.Path.Length == _temp.ParentPath.Length ? 0 : 1))), _temp.Name);
                            if (!replaceALL && File.Exists(TargetPath))
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
                            using (StreamReader sr = new StreamReader(Path.Combine(_temp.Path, _temp.Name), encoding[0], false))
                            {
                                str = sr.ReadToEnd();
                                sr.Close();
                            }
                            str = ConvertHelper.FileConvert(str, encoding, ToChinese);
                            if (!string.IsNullOrWhiteSpace(App.Settings.FileConvert.FixLabel))
                            {
                                var list = App.Settings.FileConvert.FixLabel.Split('|').ToList();
                                list.ForEach(x => {
                                    if (Path.GetExtension(_temp.Name) == x)
                                    {
                                        switch (x)
                                        {
                                            //"*.htm|*.html|*.shtm|*.shtml|*.asp|*.apsx|*.php|*.pl|*.cgi|*.js"
                                            case ".html":
                                            case ".htm":
                                            case ".php":
                                            case ".shtm":
                                            case ".shtml":
                                            case ".asp":
                                            case ".aspx":
                                                str = Regex.Replace(str, "<meta\\s*charset=\"(.*?)\"\\s*\\/?>", string.Format("<meta charset=\"{0}\">", encoding[1].WebName), RegexOptions.IgnoreCase);
                                                str = Regex.Replace(str, @"<meta\s*http-equiv\s*=\s*""Content-Type""\s*content\s*=\s*""text\/html;charset=(.*?)""\s*\/?>", string.Format(@"<meta http-equiv=""Content-Type"" content=""text/html;charset={0}"">", encoding[1].WebName), RegexOptions.IgnoreCase);
                                                str = Regex.Replace(str, @"header(""Content-Type:text/html;\s*charset=(.*?)"");", string.Format(@"header(""Content-Type:text/html; charset={0}"");", encoding[1].WebName), RegexOptions.IgnoreCase);
                                                break;

                                        }
                                    }                              
                                });
                            }
                            Directory.CreateDirectory(Path.GetDirectoryName(TargetPath));
                            using (StreamWriter sw = new StreamWriter(TargetPath, false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]))
                            {
                                sw.Write(str);
                                sw.Flush();
                            }
                        }
                    }
                    break;
                case false:
                    {
                        treeview_nodes.ForEach(x => {
                            if (x.Nodes != null)
                            {
                                var childlist = GetAllChildNode(x);
                                childlist.OrderByDescending(y => y.Generation).ToList().ForEach(y => {
                                    if (y.IsChecked)
                                    {
                                        string temp = "";
                                        string path = GetParentSum(y, ref temp);
                                        string newpath = Path.Combine(Path.GetDirectoryName(path), ConvertHelper.Convert(Path.GetFileName(path), encoding, ToChinese));
                                        try
                                        {
                                            if (y.isFile)
                                                File.Move(path, newpath);
                                            else
                                                Directory.Move(path, newpath);
                                        }
                                        catch { }
                                    }
                                });
                            }
                        });
                    }
                    break;
            }
        }
        private void Button_Clear_Clicked(object sender, RoutedEventArgs e)
        {
            FileList.Clear();
            treeview_nodes.Clear();
            treeview.ItemSources = null;
        }

        private Node GetChildPath(string path, bool searchAll, string filter)
        {
            Node temp = new Node(null);
            temp.DisplayName = path;
            var dir = Directory.GetDirectories(path);

            dir.ToList().ForEach(x =>
            {
                temp.Nodes.Add(new Node(temp)
                {
                    DisplayName = Path.GetFileName(x),
                    IsChecked = true,
                    isFile = false,
                    Nodes = searchAll ? GetChildPath(Path.Combine(path, x), searchAll, filter).Nodes : new List<Node>()
                });
            });
            filter.Split('|').ToList().ForEach(y =>
            {
                dir = Directory.GetFiles(path, y);
                dir.ToList().ForEach(x =>
                {
                    temp.Nodes.Add(new Node(temp) { DisplayName = Path.GetFileName(x), isFile = true, IsChecked = true });
                });
            });
            temp.Nodes = temp.Nodes.Distinct().ToList();
            return temp;
        }
        private string GetParentSum(Node node, ref string sum)
        {
            if (node.Generation != 1)
                sum = Path.Combine(Path.Combine(sum, GetParentSum((Node)node.Parent, ref sum)), node.DisplayName);
            else if (node.Generation == 1)
                return node.DisplayName;
            return sum;
        }
        private List<Node> GetAllChildNode(Node node)
        {
            List<Node> temp = new List<Node>();
            if (node.Nodes.Count > 0)
            {
                node.Nodes.ForEach(x => temp.AddRange(GetAllChildNode(x)));
                temp.Add(node);
            }
            else
            {
                temp.Add(node);
            }
            return temp;
        }
        private void Button_SelectFile_Clicked(object sender, RoutedEventArgs e)
        {
            OpenFileDialog fileDialog = new OpenFileDialog() { Multiselect = true, CheckFileExists = false, CheckPathExists = true, ValidateNames = false };
            fileDialog.InitialDirectory = App.Settings.FileConvert.DefaultPath;
            fileDialog.FileName = "　";
            if (fileDialog.ShowDialog() == true)
            {
                string ParentPath= Path.GetDirectoryName(fileDialog.FileNames.First());
                OutputPath = Path.GetDirectoryName(fileDialog.FileNames.First());
                if (!FileMode)
                {
                    treeview_nodes = new List<Node>() { GetChildPath(OutputPath, AccordingToChild, UseFilter ? App.Settings.FileConvert.TypeFilter : "*") };
                    treeview.ItemSources = treeview_nodes;
                    treeview_CheckedChanged(null);
                }
                else
                {
                    foreach (string str in fileDialog.FileNames)
                    {
                        if (Path.GetFileName(str) == "　" && System.IO.Directory.Exists(System.IO.Path.GetDirectoryName(str)))
                        {
                            string folderpath = System.IO.Path.GetDirectoryName(str);
                            if (UseFilter)
                                App.Settings.FileConvert.TypeFilter.Split('|').ToList().ForEach(filter =>
                                {
                                    List<string> childFileList = System.IO.Directory.GetFiles(folderpath, filter.Trim(), AccordingToChild ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly).ToList();
                                    childFileList.ForEach(x =>
                                    {
                                        FileList.Add(new FileList_Line() { isChecked = true, isFile = true, Name = System.IO.Path.GetFileName(x), ParentPath = ParentPath, Path = Path.GetDirectoryName(x) });
                                    });
                                });
                            else
                            {
                                List<string> childFileList = System.IO.Directory.GetFiles(folderpath, "*", AccordingToChild ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly).ToList();
                                childFileList.ForEach(x =>
                                {
                                    FileList.Add(new FileList_Line() { isChecked = true, isFile = true, Name = System.IO.Path.GetFileName(x), ParentPath = ParentPath, Path = Path.GetDirectoryName(x) });
                                });
                            }
                            FileList = new ObservableCollection<FileList_Line>(FileList.OrderBy(x => x.Name).Distinct().OrderBy(x => x.isFile).OrderBy(x => x.Path));
                        }
                        else if (File.Exists(str))
                        {
                            FileList.Add(new FileList_Line() { isChecked = true, Name = Path.GetFileName(str), ParentPath = ParentPath, Path = Path.GetDirectoryName(str) });
                        }
                    }
                }
            }
        }
        private string _ClipBoard = "";
        public string ClipBoard { get => _ClipBoard; set { _ClipBoard = value; OnPropertyChanged(); } }
        private string _InputPreviewText = "";
        public string InputPreviewText { get => _InputPreviewText; set { _InputPreviewText = value; OnPropertyChanged(); } }
        private string _OutputPreviewText = "";
        public string OutputPreviewText { get => _OutputPreviewText; set { _OutputPreviewText = value; OnPropertyChanged(); } }
        private bool _FileMode = true;
        public bool FileMode { get => _FileMode; set { _FileMode = value; OnPropertyChanged(); } }
        private bool _AccordingToChild = true;
        public bool AccordingToChild { get => _AccordingToChild; set { _AccordingToChild = value; OnPropertyChanged(); } }
        private bool _UseFilter = true;
        public bool UseFilter { get => _UseFilter; set { _UseFilter = value; OnPropertyChanged(); App.Settings.FileConvert.UseFilter = value; App.Save(); } }
        private string _OutputPath = "";
        public string OutputPath { get => _OutputPath; set { _OutputPath = value; OnPropertyChanged(); } }


        private ObservableCollection<FileList_Line> _FileList = new ObservableCollection<FileList_Line>();

        public ObservableCollection<FileList_Line> FileList { get => _FileList; set { _FileList = value; OnPropertyChanged(); } }

        List<Node> treeview_nodes = new List<Node>();
        private void treeview_CheckedChanged(CheckBox sender)
        {
            StringBuilder sb = new StringBuilder();
            StringBuilder sb2 = new StringBuilder();
            treeview_nodes.ForEach(x => {
                if (x.Nodes != null)
                {
                    var childlist = GetAllChildNode(x);
                    childlist.OrderByDescending(y => y.Generation).ToList().ForEach(y => {
                        if (y.IsChecked)
                        {
                            string temp = "";
                            string path = GetParentSum(y, ref temp);
                            string newpath = Path.Combine(Path.GetDirectoryName(path), ConvertHelper.Convert(Path.GetFileName(path), encoding, ToChinese));
                            sb.AppendLine(path);
                            sb2.AppendLine(newpath);
                        }
                    });
                }
            });
            InputPreviewText = sb.ToString();
            OutputPreviewText = sb2.ToString();
        }
        public class FileList_Line
        {
            public int ID { get; set; }
            public bool isChecked { get; set; }     //or IsSelected maybe? whichever name you want  
            public bool isFile { get; set; }
            public string Name { get; set; }
            public string ParentPath { get; set; }
            public string Path { get; set; }
        }
        public event PropertyChangedEventHandler PropertyChanged;
        protected virtual void OnPropertyChanged([CallerMemberName] string propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
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
        private void listview_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            treeview_nodes.Clear();
            treeview.ItemSources = null;
            if (listview.SelectedItem != null)
            {
                FileList_Line line = ((FileList_Line)listview.SelectedItem);
                string path = Path.Combine(line.Path, line.Name);
                if (File.Exists(path))
                {
                    using (StreamReader sr = new StreamReader(path, encoding[0], false))
                    {
                        char[] c = null;
                        if (sr.Peek() >= 0)
                        {
                            c = new char[App.Settings.MaxLengthPreview * 1000];
                            sr.ReadBlock(c, 0, c.Length);
                            StringBuilder @string = new StringBuilder(c.Length);
                            @string.Append(c);
                            InputPreviewText = @string.ToString().TrimEnd('\0');
                        }
                    }
                    OutputPreviewText = ConvertHelper.FileConvert(InputPreviewText, encoding, ToChinese);
                }
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
            ModeChange(null, null);
        }

        private void ModeChange(object sender, RoutedEventArgs e)
        {
            if (FileMode)
            {
                switchPage(Page1, Page2);
                listview_SelectionChanged(null, null);
            }
            else
            {
                switchPage(Page2, Page1);
                treeview_CheckedChanged(null);
            }
        }
        Grid g;
        private void switchPage(Grid preGide, Grid nextGrid)
        {
            preGide.Visibility = Visibility.Collapsed;
            g = nextGrid;
            Canvas.SetLeft(g, 0);
            Canvas.SetTop(g, 0);
            nextGrid.Visibility = Visibility.Visible;
            g.BringIntoView();
        }
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
