using ConvertZZ.Moudle;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows;

namespace ConvertZZ
{
    /// <summary>
    /// App.xaml 的互動邏輯
    /// </summary>
    public partial class App : Application
    {
        public static System.Windows.Forms.NotifyIcon nIcon = new System.Windows.Forms.NotifyIcon();
        public static bool DicLoaded { get; set; } = false;
        public App()
        {
            
        }

        public static ChineseConverter ChineseConverter { get; set; } = new ChineseConverter();

        private void Application_Startup(object sender, StartupEventArgs e)
        {
            App.Reload(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "ConvertZZ.json"));
            Task.Run(() =>
            {
                foreach (string p in System.IO.Directory.GetFiles(System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Dictionary")))
                    ChineseConverter.Load(p);
                ChineseConverter.ReloadFastReplaceDic();
                DicLoaded = true;
            });

            ShutdownMode = ShutdownMode.OnMainWindowClose;
            if (e.Args.Length > 0)
            {
                Encoding[] encoding = new Encoding[2];
                bool EncodingSetted = false;
                int ToChinese = 0;
                string path1 = null, path2 = null;
                Regex Regex_path1 = null;
                int VocabularyCorrection = -1;
                for (int i = 0; i < e.Args.Length; i++)
                {
                    switch (e.Args[i])
                    {
                        case "/i:ule":
                            encoding[0] = Encoding.Unicode;
                            EncodingSetted = true;
                            break;
                        case "/i:ube":
                            encoding[0] = Encoding.BigEndianUnicode;
                            EncodingSetted = true;
                            break;
                        case "/i:utf8":
                            encoding[0] = Encoding.UTF8;
                            EncodingSetted = true;
                            break;
                        case "/i:gbk":
                            encoding[0] = Encoding.GetEncoding("GBK");
                            EncodingSetted = true;
                            break;
                        case "/i:big5":
                            encoding[0] = Encoding.GetEncoding("Big5");
                            EncodingSetted = true;
                            break;
                        case "/o:ule":
                            encoding[1] = Encoding.Unicode;
                            break;
                        case "/o:ube":
                            encoding[1] = Encoding.BigEndianUnicode;
                            break;
                        case "/o:utf8":
                            encoding[1] = Encoding.UTF8;
                            break;
                        case "/o:gbk":
                            encoding[1] = Encoding.GetEncoding("GBK");
                            break;
                        case "/o:big5":
                            encoding[1] = Encoding.GetEncoding("Big5");
                            break;
                        case "/f:t":
                            ToChinese = 1;
                            break;
                        case "/f:s":
                            ToChinese = 2;
                            break;
                        case "/f:d":
                            ToChinese = 0;
                            break;
                        case "/d:t":
                            VocabularyCorrection = 1;
                            break;
                        case "/d:f":
                            VocabularyCorrection = 0;
                            break;
                        case "/d:s":
                            VocabularyCorrection = -1;
                            break;
                        default:
                            if (path1 == null)
                            {
                                path1 = e.Args[i];
                                Regex_path1 = new Regex(Regex.Replace(path1.Replace("*", "(.*?)"),"[\\/]","[\\\\/]") + "$");
                            }
                            else
                            {
                                path2 = e.Args[i];
                            }
                            break;
                    }
                }
                string s = "";
                List<string> file = new List<string>();
                bool ModeIsOneFile = true;
                if (path1.Contains("*"))
                {
                    ModeIsOneFile = false;
                    Directory.GetFiles(Path.GetDirectoryName(path1), Path.GetFileName(path1)).ToList().ForEach(x =>
                        file.Add(x)
                    );
                }
                else
                {
                    if (File.Exists(path1))
                        file.Add(path1);
                    else
                    {
                        Console.WriteLine($"檔案\"{path1}\" 不存在");
                        Console.Read();
                        Shutdown(1);
                        return;
                    }
                }
                if (encoding[1] == null || string.IsNullOrWhiteSpace(path1))
                {
                    Console.WriteLine("參數錯誤(目標編碼為空或來源檔案路徑未填寫)");
                    Console.Read();
                    Shutdown(1);
                    return;
                }
                if (string.IsNullOrWhiteSpace(path2))
                    path2 = path1;
                if (path1.Count(x => x == '*') != path2.Count(x => x == '*') && path2.Contains("*"))
                {
                    Console.WriteLine("參數錯誤(輸出路徑的萬用字元術與輸入路徑不同)");
                    Console.Read();
                    Shutdown(1);
                    return;
                }
                if (path1.Contains("*") && !path2.Contains("*") && File.Exists(path2))
                {
                    Console.WriteLine("參數錯誤(輸入路徑具有萬用字元，但輸出路徑卻指向一個檔案)");
                    Console.Read();
                    Shutdown(1);
                    return;
                }
                foreach (var f in file)
                {
                    using (Stream stream = new FileStream(f, FileMode.Open, FileAccess.Read, FileShare.Read))
                    {
                        using (StreamReader streamReader = new StreamReader(stream, encoding[0], false))
                        {
                            s = streamReader.ReadToEnd();
                        }
                    }
                    if (!EncodingSetted)
                        switch (EncodingAnalyzer.Analyze(s))
                        {
                            case -1:
                                encoding[0] = Encoding.Default;
                                break;
                            case 0:
                            case 1:
                                encoding[0] = Encoding.GetEncoding("BIG5");
                                break;
                            case 2:
                            case 3:
                                encoding[0] = Encoding.GetEncoding("GBK");
                                break;
                        }
                    s = ConvertHelper.FileConvert(s, encoding, ToChinese, VocabularyCorrection);
                    if (ModeIsOneFile)
                    {
                        using (StreamWriter streamWriter = new StreamWriter(path2, false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]))
                        {
                            streamWriter.Write(s);
                            streamWriter.Flush();
                        }
                    }
                    else
                    {
                        var m1 = Regex_path1.Match(f);
                        if(m1.Success)
                        {
                            if (path2.Contains("*"))
                            {
                                var array = path2.Split('*');
                                string @string = "";
                                for (int i = 0; i < array.Length; i++)
                                {
                                    @string += array[i];
                                    if (i + 1 <= m1.Groups.Count - 1)
                                        @string += m1.Groups[i + 1].Value;
                                }

                                Directory.CreateDirectory(Path.GetDirectoryName(@string));
                                using (StreamWriter streamWriter = new StreamWriter(@string, false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]))
                                {
                                    streamWriter.Write(s);
                                    streamWriter.Flush();
                                }
                            }
                            else
                            {
                                Directory.CreateDirectory(Path.GetDirectoryName(path2));
                                using (StreamWriter streamWriter = new StreamWriter(Path.Combine(Path.GetDirectoryName(path2), Path.GetFileName(f)), false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]))
                                {
                                    streamWriter.Write(s);
                                    streamWriter.Flush();
                                }
                            }
                        }
                    }
                }
                Shutdown(1);
                return;
            }
            else
            {
                if ((Process.GetProcessesByName(Process.GetCurrentProcess().ProcessName).Length > 1))
                {
                    MessageBox.Show("應用程式 " + Process.GetCurrentProcess().ProcessName + " 己在執行中，請先關閉舊視窗。", "警告", MessageBoxButton.OK, MessageBoxImage.Warning);
                    Shutdown(1);
                    return;
                }
                nIcon.Icon = ConvertZZ.Properties.Resources.icon;
                nIcon.Visible = true;
                if (Settings.CheckVersion)
                {
                    Task.Run(() =>
                    {
                        var versionReport = UpdateChecker.ChecktVersion();
                        if (versionReport != null && versionReport.HaveNew)
                        {
                            if (MessageBox.Show(String.Format("發現新版本{0}(目前版本：{1})\r\n前往官網下載更新？", versionReport.Newst.ToString(), versionReport.Current.ToString()), "發現更新", MessageBoxButton.YesNo) == MessageBoxResult.Yes)
                            {
                                Process.Start("https://github.com/flier268/ConvertZZ/releases");
                            }
                        }
                    });
                }
                MainWindow window = new MainWindow();
                MainWindow = window;
                window.Show();
            }
        }
    }
}
