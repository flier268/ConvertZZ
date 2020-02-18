using ConvertZZ.Moudle;
using Microsoft.Win32;
using Microsoft.WindowsAPICodePack.Dialogs;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;

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
        public Page_File(string[] FileNames) : this()
        {
            if (FileNames == null)
                return;
            OutputPath = Path.GetDirectoryName(FileNames.First());
            ImportFileNames(FileNames);
            Combobox_Filter_SelectionChanged(Combobox_Filter, null);
        }
        /// <summary>
        /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
        /// </summary>
        Encoding[] encoding = new Encoding[2] { Encoding.UTF8, Encoding.UTF8 };
        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        int ToChinese = 0;

        private void ResetConvertButton(Button button)
        {
            DismissButtonProgress = 0;
            Dispatcher.BeginInvoke(new Action(() =>
            {
                Mouse.OverrideCursor = null;
                button.Content = "轉換";
                Listview_SelectionChanged(null, null);
                InputPreviewText = "";
                OutputPreviewText = "";
            }), DispatcherPriority.SystemIdle);
        }

        CancellationTokenSource cts = new CancellationTokenSource();
        private async void Button_Convert_ClickAsync(object sender, RoutedEventArgs e)
        {
            if ((string)((Button)e.Source).Content == "停止中...")
                return;
            else if ((string)((Button)e.Source).Content == "停止")
            {
                cts.Cancel();
                ((Button)e.Source).Content = "停止中...";
                return;
            }
            else if ((string)((Button)e.Source).Content == "轉換")
            {
                cts = new CancellationTokenSource();
                ((Button)e.Source).Content = "停止";
            }
            Stopwatch stopwatch = new Stopwatch();
            StringBuilder AppendLog = new StringBuilder();
            switch (FileMode)
            {
                case true:
                    {
                        FileList.ToList().ForEach(x => x.IsReplace = false);
                        var temp = FileList.Where(x => x.IsChecked).ToList();
                        double count_total = temp.Count, count_current = 0.0;
                        DismissButtonProgress = 0;
                        bool replaceALL = false;
                        bool skip = false;
                        foreach (var _temp in temp)
                        {
                            string TargetPath = Path.Combine(Path.Combine(OutputPath, _temp.Path.Substring(_temp.ParentPath.Length + (_temp.Path.Length == _temp.ParentPath.Length ? 0 : 1))), _temp.Name);
                            if (!replaceALL && File.Exists(TargetPath))
                            {
                                if (!skip)
                                {
                                    switch (Moudle.Window_MessageBoxEx.ShowDialog(string.Format("{0}發生檔名衝突，是否取代?", _temp.Name), "警告", "取代", "略過", "取消", "套用到全部"))
                                    {
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.A:
                                            break;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.B:
                                            continue;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.C:
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.CO:
                                            DismissButtonProgress = 100.0;
                                            ResetConvertButton((Button)e.Source);
                                            listview.SelectedIndex = -1;                                            
                                            return;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.AO:
                                            replaceALL = true;
                                            break;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.BO:
                                            skip = true;
                                            continue;
                                        case Moudle.Window_MessageBoxEx.MessageBoxExResult.NONE:
                                            continue;
                                    }
                                }
                                else
                                    continue;
                            }
                            _temp.IsReplace = true;
                        }
                        temp = FileList.Where(x => x.IsChecked && x.IsReplace).ToList();
                        count_current = count_total - temp.Count;
                        DismissButtonProgress = count_current / count_total * 100.0;
                        Mouse.OverrideCursor = Cursors.Wait;
                        stopwatch.Start();
                        foreach (var _temp in temp)
                        {
                            if (cts.IsCancellationRequested) { ResetConvertButton((Button)e.Source); return; }
                            string TargetPath = Path.Combine(Path.Combine(OutputPath, _temp.Path.Substring(_temp.ParentPath.Length + (_temp.Path.Length == _temp.ParentPath.Length ? 0 : 1))), _temp.Name);

                            string str = "";
                            using (StreamReader sr = new StreamReader(Path.Combine(_temp.Path, _temp.Name), encoding[0], false))
                            {
                                str = sr.ReadToEnd();
                                sr.Close();
                            }
                            try
                            {
                                str = await ConvertHelper.ConvertAsync(str, ToChinese);
                                if (!string.IsNullOrWhiteSpace(App.Settings.FileConvert.FixLabel))
                                {
                                    var list = App.Settings.FileConvert.FixLabel.Split('|').Select(x => x.ToLower()).ToList();
                                    list.ForEach(x =>
                                    {
                                        if (Path.GetExtension(_temp.Name).ToLower() == x)
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
                                                    //html5
                                                    str = Regex.Replace(str, "<meta(\\s+.*?)charset=\"(.*?)\"(.*?)>", string.Format("<meta$1charset=\"{0}\"$3>", encoding[1].WebName), RegexOptions.IgnoreCase);
                                                    //html4
                                                    str = Regex.Replace(str, "<meta\\s+(.*?)content=\"(.*?)charset=(.*?)\"(.*?)>", string.Format("<meta $1content=\"$2charset={0}\"$4>", encoding[1].WebName), RegexOptions.IgnoreCase);
                                                    //php
                                                    str = Regex.Replace(str, @"header(""Content-Type:text/html;\s*charset=(.*?)"");", string.Format(@"header(""Content-Type:text/html; charset={0}"");", encoding[1].WebName), RegexOptions.IgnoreCase);
                                                    break;
                                                case "css":
                                                    str = Regex.Replace(str, "@charset \"(.*?)\"", string.Format("@charset \"{0}\"", encoding[1].WebName), RegexOptions.IgnoreCase);
                                                    break;
                                            }
                                        }
                                    });
                                }
                                if (cts.IsCancellationRequested) { ResetConvertButton((Button)e.Source); return; }
                                Directory.CreateDirectory(Path.GetDirectoryName(TargetPath));
                                using (StreamWriter sw = new StreamWriter(TargetPath, false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]))
                                {
                                    sw.Write(str);
                                    sw.Flush();
                                }
                            }
                            catch (Exception ex)
                            {
                                AppendLog.AppendLine($"[Error][{ex.Message}] \"{Path.Combine(_temp.Path, _temp.Name)}\"");
                            }
                            finally
                            {
                                count_current++;
                                DismissButtonProgress = count_current / count_total * 100.0;
                            }
                        }
                        DismissButtonProgress = 0.0;
                        stopwatch.Stop();
                        Mouse.OverrideCursor = null;
                    }
                    break;
                case false:
                    {
                        stopwatch.Start();

                        try
                        {
                            ConvertFolderAndFileName(true);
                        }
                        catch (NullReferenceException ex)
                        {
                            AppendLog.AppendLine(ex.Message);
                        }
                        stopwatch.Stop();
                    }
                    break;
            }
            if (AppendLog.Length != 0)
                Window_MessageBoxEx.ShowDialog(AppendLog.ToString(), "轉換過程中出現錯誤", "我知道了");
            else if (App.Settings.Prompt)
            {
                new Toast(string.Format("轉換完成\r\n耗時：{0} ms", stopwatch.ElapsedMilliseconds)).Show();
            }
            ResetConvertButton((Button)e.Source);
        }
        private void Button_Clear_Clicked(object sender, RoutedEventArgs e)
        {
            FileList.Clear();
            FileListTemp.Clear();
            treeview_nodes.Clear();
            treeview.ItemSources = null;
            InputPreviewText = "";
            OutputPreviewText = "";
        }

        private Node GetChildPath(string path, bool searchAll, string filter)
        {
            Node temp = new Node(null)
            {
                DisplayName = path
            };
            List<string> dir;
            try
            {
                dir = Directory.GetDirectories(path).ToList();
                dir.ForEach(x =>
                {
                    var y = new Node(temp)
                    {
                        DisplayName = Path.GetFileName(x),
                        IsChecked = true,
                        IsFile = false,
                        Nodes = searchAll ? GetChildPath(Path.Combine(path, x), searchAll, filter).Nodes : new List<Node>()
                    };
                    y.RegistPropertyChangedEvent();
                    temp.Nodes.Add(y);
                });
                dir = Directory.GetFiles(path).ToList();
                dir.ForEach(x =>
                {
                    var y = new Node(temp) { DisplayName = Path.GetFileName(x), IsFile = true, IsChecked = true };
                    y.RegistPropertyChangedEvent();
                    temp.Nodes.Add(y);
                });
            }
            catch
            {
                throw;
            }
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
        private void GetPathParts(Node node, ref Dictionary<string, string> sum)
        {
            string Key;
            if (node.Generation == 1)
            {
                Key = Path.GetFileName(node.DisplayName);
                sum[Key] = Key;
            }
            else
            {
                Key = node.DisplayName;
                sum[node.DisplayName] = Key;
            }
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
        private async void Button_SelectFile_Clicked(object sender, RoutedEventArgs e)
        {
            OpenFileDialog fileDialog = new OpenFileDialog() { Multiselect = true, CheckFileExists = false, CheckPathExists = true, ValidateNames = false };
            fileDialog.InitialDirectory = App.Settings.FileConvert.DefaultPath;
            fileDialog.FileName = "　";
            if (FileMode)
                fileDialog.Filter = Combobox_Filter.SelectedValue.ToString();
            if (fileDialog.ShowDialog() == true)
            {
                OutputPath = Path.GetDirectoryName(fileDialog.FileNames.First());
                if (!FileMode)
                {
                    string a;
                    try
                    {
                        treeview_nodes = new List<Node>() { GetChildPath(OutputPath, AccordingToChild, Combobox_Filter.Text) };
                        a = await CreateDictionary();
                    }
                    catch (Exception ex)
                    {
                        a = $"[Error][{ex.Message}]StackTrace: {ex.StackTrace}";
                    }
                    if (!string.IsNullOrWhiteSpace(a))
                    {
                        PathParts = null;
                        Window_MessageBoxEx.ShowDialog(a, "預覽轉換中出現錯誤", "我知道了");
                    }
                    treeview_nodes.ForEach(x => x.PropertyChanged += Treeview_CheckedChanged);
                    treeview.ItemSources = treeview_nodes;
                    Treeview_CheckedChanged(null, new PropertyChangedEventArgs(nameof(Node.IsChecked)));
                }
                else
                {
                    ImportFileNames(fileDialog.FileNames);
                }
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

        public string ClipBoard { get; set; } = "";

        public string InputPreviewText { get; set; } = "";

        public string OutputPreviewText { get; set; } = "";

        public bool FileMode { get; set; } = true;

        public bool AccordingToChild { get; set; } = true;

        public string OutputPath { get; set; } = "";

        public double DismissButtonProgress { get; set; }

        public ObservableCollection<FileList_Line> FileList { get; set; } = new ObservableCollection<FileList_Line>();
        public bool UnicodeAddBom { get => App.Settings.FileConvert.UnicodeAddBom; set { App.Settings.FileConvert.UnicodeAddBom = value; App.Save(); } }


        List<Node> treeview_nodes = new List<Node>();
        Dictionary<string, string> PathParts = new Dictionary<string, string>();
        private void Treeview_CheckedChanged(object sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName != nameof(Node.IsChecked))
                return;
            var r = ConvertFolderAndFileName(false);
            InputPreviewText = r[0];
            OutputPreviewText = r[1];
        }
        private async Task<string> CreateDictionary()
        {
            PathParts = PathParts ?? new Dictionary<string, string>();
            PathParts.Clear();
            StringBuilder AppendLog = new StringBuilder();
            foreach (var x in treeview_nodes)
            {
                if (x.Nodes != null)
                {
                    GetPathParts(x, ref PathParts);
                    var childlist = GetAllChildNode(x);
                    foreach (var y in childlist)
                    {
                        GetPathParts(y, ref PathParts);
                    }
                }
            }
            try
            {
                PathParts = await ConvertHelper.ConvertDictionary(PathParts, encoding, ToChinese);
            }
            catch (Fanhuaji_API.Fanhuaji.FanhuajiException ex)
            {
                AppendLog.AppendLine($"[Error][{ex.Message}]");
                return AppendLog.ToString();
            }
            catch (Exception ex)
            {
                AppendLog.AppendLine($"[Error][{ex.Message}]StackTrace: {ex.StackTrace}");
                return AppendLog.ToString();
            }
            return AppendLog.ToString();
        }
        private string[] ConvertFolderAndFileName(bool Rename)
        {
            string InputText, OutputText;
            StringBuilder sb = new StringBuilder();
            StringBuilder sb2 = new StringBuilder();
            StringBuilder AppendLog = new StringBuilder();
            foreach (var x in treeview_nodes)
            {
                if (x.Nodes != null)
                {
                    var temps = GetAllChildNode(x).Where(y => y.IsChecked).Reverse().ToList();
                    foreach (var y in temps)
                    {
                        string temp = "";
                        string path = GetParentSum(y, ref temp);
                        sb.AppendLine(path);
                    }
                }
            }
            InputText = sb.ToString();
            if (PathParts == null)
            {
                OutputText = "轉換時出現錯誤，請檢查網路及設定";
                if (Rename)
                    throw new NullReferenceException(OutputText);
                else
                    return new string[] { InputText, OutputText };
            }


            var treeview_nodes_output = treeview_nodes.Select(x => x.Clone() as Node).ToList();
            string RenameError = RenameFolderAndFileName(treeview_nodes_output, PathParts, Rename);
            if (!string.IsNullOrWhiteSpace(RenameError))
                AppendLog.AppendLine(RenameError);

            foreach (var x in treeview_nodes_output)
            {
                if (x.Nodes != null)
                {
                    var temps = GetAllChildNode(x).Where(y => y.IsChecked).Reverse().ToList();
                    foreach (var y in temps)
                    {
                        string temp = "";
                        sb2.AppendLine(GetParentSum(y, ref temp));
                    }
                }
            }


            if (AppendLog.Length != 0)
            {
                Window_MessageBoxEx.ShowDialog(AppendLog.ToString(), "轉換過程中出現錯誤", "我知道了");
                OutputText = "";
            }
            else
                OutputText = sb2.ToString();
            return new string[] { InputText, OutputText };
        }

        private string RenameFolderAndFileName(List<Node> nodes, Dictionary<string, string> Dictionary, bool Rename)
        {
            StringBuilder @string = new StringBuilder();
            nodes.ForEach(x =>
            {
                if (x.IsChecked)
                {
                    if (Rename)
                    {
                        string temp = "";
                        string path = GetParentSum(x, ref temp);
                        string newpath = Path.Combine(Path.GetDirectoryName(path), GetNewDisplayName(x.IsFile, Path.GetFileName(path)));
                        try
                        {
                            if (path != newpath)
                            {
                                if (x.IsFile)
                                    File.Move(path, newpath);
                                else
                                    Directory.Move(path, newpath);
                            }
                        }
                        catch (Exception ex)
                        {
                            @string.AppendLine($"[Error][{ex.Message}]StackTrace: {ex.StackTrace}");
                        }
                    }

                    if (x.Generation == 1)
                        x.DisplayName = Path.Combine(Path.GetDirectoryName(x.DisplayName), GetNewDisplayName(x.IsFile, Path.GetFileName(x.DisplayName)));
                    else
                        x.DisplayName = GetNewDisplayName(x.IsFile, x.DisplayName);
                    string GetNewDisplayName(bool IsFile, string FileName)
                    {
                        return IsFile ? MakeFilenameValid(Dictionary[FileName]) : MakeFoldernameValid(Dictionary[FileName]);
                    }
                }
                @string.AppendLine(RenameFolderAndFileName(x.Nodes, Dictionary, Rename));
            });
            return @string.ToString();
        }

        public class FileList_Line
        {
            public int ID { get; set; }
            public bool IsChecked { get; set; }     //or IsSelected maybe? whichever name you want  
            public bool IsFile { get; set; }
            public string Name { get; set; }
            public string ParentPath { get; set; }
            public string Path { get; set; }
            public bool IsReplace { get; set; }
        }
        public event PropertyChangedEventHandler PropertyChanged;




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
        private async void Listview_SelectionChanged(object sender, SelectionChangedEventArgs e)
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
                    try
                    {
                        OutputPreviewText = await ConvertHelper.FileConvert(InputPreviewText, encoding, ToChinese);
                    }
                    catch (Fanhuaji_API.Fanhuaji.FanhuajiException ex)
                    {
                        OutputPreviewText = $"[Error][{ex.Message}] \"{path}\"";
                    }
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

        private async void ModeChange(object sender, RoutedEventArgs e)
        {
            if (FileMode)
            {
                SwitchPage(Page1, Page2);
                Listview_SelectionChanged(null, null);
            }
            else
            {
                SwitchPage(Page2, Page1);
                var a = await CreateDictionary();
                if (!string.IsNullOrWhiteSpace(a))
                {
                    PathParts = null;
                    Window_MessageBoxEx.ShowDialog(a, "預覽轉換中出現錯誤", "我知道了");
                }
                Treeview_CheckedChanged(null, new PropertyChangedEventArgs(nameof(Node.IsChecked)));
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
        #region
        private static string MakeFilenameValid(string filename)
        {
            if (filename == null)
                throw new ArgumentNullException();

            if (filename.EndsWith("."))
                filename = Regex.Replace(filename, @"\.+$", "");

            if (filename.Length == 0)
                throw new ArgumentException();

            if (filename.Length > 245)
                filename = filename.Take(245).ToString();

            foreach (char c in System.IO.Path.GetInvalidFileNameChars())
            {
                filename = filename.Replace(c, '_');
            }

            return filename;
        }

        private static string MakeFoldernameValid(string foldername)
        {
            if (foldername == null)
                throw new ArgumentNullException();

            if (foldername.EndsWith("."))
                foldername = Regex.Replace(foldername, @"\.+$", "");

            if (foldername.Length == 0)
                throw new ArgumentException();

            if (foldername.Length > 245)
                foldername = foldername.Take(245).ToString();

            foreach (char c in System.IO.Path.GetInvalidPathChars())
            {
                foldername = foldername.Replace(c, '_');
            }

            return foldername;
        }
        #endregion
    }
}
