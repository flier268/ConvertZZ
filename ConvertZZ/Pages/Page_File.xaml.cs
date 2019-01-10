using ConvertZZ.Moudle;
using Microsoft.Win32;
using Microsoft.WindowsAPICodePack.Dialogs;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

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
            DataContext = this;
            OutputPath = App.Settings.FileConvert.DefaultPath.Substring(App.Settings.FileConvert.DefaultPath[0] == '!' ? 1 : 0);
            Combobox_Filter.ItemsSource = App.Settings.FileConvert.GetFilterList();
            Combobox_Filter.SelectedIndex = 0;
        }
        /// <summary>
        /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
        /// </summary>
        Encoding[] encoding = new Encoding[2] { Encoding.UTF8, Encoding.UTF8 };
        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        int ToChinese = 0;
        private async void Button_Convert_ClickAsync(object sender, RoutedEventArgs e)
        {
            ((Button)e.Source).IsEnabled = false;
            Stopwatch stopwatch = new Stopwatch();
            switch (FileMode)
            {
                case true:
                    {
                        var temp = FileList.Where(x => x.IsChecked).ToList();
                        bool replaceALL = false;
                        bool skip = false;
                        Mouse.OverrideCursor = Cursors.Wait;
                        stopwatch.Start();
                        foreach (var _temp in temp)
                        {
                            string TargetPath = Path.Combine(Path.Combine(OutputPath, _temp.Path.Substring(_temp.ParentPath.Length + (_temp.Path.Length == _temp.ParentPath.Length ? 0 : 1))), _temp.Name);
                            if (!replaceALL && File.Exists(TargetPath))
                            {
                                if (!skip)
                                {
                                    Mouse.OverrideCursor = null;
                                    stopwatch.Stop();
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
                                    Mouse.OverrideCursor = Cursors.Wait;
                                    stopwatch.Start();
                                }
                                else
                                    continue;
                            }
                            await Task.Run(() =>
                            {
                                string str = "";
                                using (StreamReader sr = new StreamReader(Path.Combine(_temp.Path, _temp.Name), encoding[0], false))
                                {
                                    str = sr.ReadToEnd();
                                    sr.Close();
                                }
                                str = ConvertHelper.Convert(str, ToChinese);
                                if (!string.IsNullOrWhiteSpace(App.Settings.FileConvert.FixLabel))
                                {
                                    var list = App.Settings.FileConvert.FixLabel.Split('|').ToList();
                                    list.ForEach(x =>
                                    {
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
                            });
                        }
                        stopwatch.Stop();
                        Mouse.OverrideCursor = null;
                    }
                    break;
                case false:
                    {
                        stopwatch.Start();
                        treeview_nodes.ForEach(x =>
                        {
                            if (x.Nodes != null)
                            {
                                var childlist = GetAllChildNode(x);
                                childlist.OrderByDescending(y => y.Generation).ToList().ForEach(y =>
                                {
                                    if (y.IsChecked)
                                    {
                                        string temp = "";
                                        string path = GetParentSum(y, ref temp);
                                        string newpath = Path.Combine(Path.GetDirectoryName(path), ConvertHelper.Convert(Path.GetFileName(path), encoding, ToChinese));
                                        try
                                        {
                                            if (y.IsFile)
                                                File.Move(path, newpath);
                                            else
                                                Directory.Move(path, newpath);
                                        }
                                        catch { }
                                    }
                                });
                            }
                        });
                        stopwatch.Stop();
                    }
                    break;
            }
            if (App.Settings.Prompt)
            {
                MessageBox.Show(string.Format("轉換完成\r\n耗時：{0} ms", stopwatch.ElapsedMilliseconds));
            }
            ((Button)e.Source).IsEnabled = true;
            Listview_SelectionChanged(null, null);
        }
        private void Button_Clear_Clicked(object sender, RoutedEventArgs e)
        {
            FileList.Clear();
            FileListTemp.Clear();
            treeview_nodes.Clear();
            treeview.ItemSources = null;
        }

        private Node GetChildPath(string path, bool searchAll, string filter)
        {
            Node temp = new Node(null)
            {
                DisplayName = path
            };
            var dir = Directory.GetDirectories(path);

            dir.ToList().ForEach(x =>
            {
                temp.Nodes.Add(new Node(temp)
                {
                    DisplayName = Path.GetFileName(x),
                    IsChecked = true,
                    IsFile = false,
                    Nodes = searchAll ? GetChildPath(Path.Combine(path, x), searchAll, filter).Nodes : new List<Node>()
                });
            });
            dir = Directory.GetFiles(path);
            dir.ToList().ForEach(x =>
            {
                temp.Nodes.Add(new Node(temp) { DisplayName = Path.GetFileName(x), IsFile = true, IsChecked = true });
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
            if (FileMode)
                fileDialog.Filter = Combobox_Filter.SelectedValue.ToString();
            if (fileDialog.ShowDialog() == true)
            {
                string ParentPath = Path.GetDirectoryName(fileDialog.FileNames.First());
                OutputPath = Path.GetDirectoryName(fileDialog.FileNames.First());
                if (!FileMode)
                {
                    treeview_nodes = new List<Node>() { GetChildPath(OutputPath, AccordingToChild, Combobox_Filter.Text) };
                    treeview.ItemSources = treeview_nodes;
                    Treeview_CheckedChanged(null);
                }
                else
                {
                    foreach (string str in fileDialog.FileNames)
                    {
                        if (Path.GetFileNameWithoutExtension(str) == "　" && System.IO.Directory.Exists(System.IO.Path.GetDirectoryName(str)))
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
                Combobox_Filter_SelectionChanged(Combobox_Filter, null);
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
        private string _OutputPath = "";
        public string OutputPath { get => _OutputPath; set { _OutputPath = value; OnPropertyChanged(); } }


        private ObservableCollection<FileList_Line> _FileList = new ObservableCollection<FileList_Line>();

        public ObservableCollection<FileList_Line> FileList { get => _FileList; set { _FileList = value; OnPropertyChanged(); } }

        List<Node> treeview_nodes = new List<Node>();
        private void Treeview_CheckedChanged(CheckBox sender)
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
            public bool IsChecked { get; set; }     //or IsSelected maybe? whichever name you want  
            public bool IsFile { get; set; }
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
        private void Listview_SelectionChanged(object sender, SelectionChangedEventArgs e)
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
                SwitchPage(Page1, Page2);
                Listview_SelectionChanged(null, null);
            }
            else
            {
                SwitchPage(Page2, Page1);
                Treeview_CheckedChanged(null);
            }
        }
        Grid g;
        private void SwitchPage(Grid preGide, Grid nextGrid)
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
            var dlg = new CommonOpenFileDialog
            {
                Title = "Select Output folder",
                IsFolderPicker = true,
                AddToMostRecentlyUsedList = false,
                AllowNonFileSystemItems = false,
                EnsureFileExists = true,
                EnsurePathExists = true,
                EnsureReadOnly = false,
                EnsureValidNames = true,
                Multiselect = false,
                ShowPlacesList = true
            };

            if (dlg.ShowDialog() == CommonFileDialogResult.Ok)
            {
                OutputPath = dlg.FileName;
            }
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
