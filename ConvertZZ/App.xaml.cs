using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Linq;
using System.Security.Principal;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Core.Services.EncodingConverter;
using ConvertZZ.Core.Services.TextConverter;
using ConvertZZ.Moudle;
using ConvertZZ.Services;
using ConvertZZ.Views;
using Fanhuaji_API;

namespace ConvertZZ
{
    /// <summary>
    /// App.xaml 的互動邏輯
    /// </summary>
    public partial class App : Application
    {
        public static System.Windows.Forms.NotifyIcon NIcon { get; } = new System.Windows.Forms.NotifyIcon();
        public static App Instance { get; private set; } = null!;

        #region Converter

        public ITextConverter TextConverter { get; set; }
        public CommonEncodingConverter CommonEncodingConverter { get; set; }
        public GbkToBig5EncodingConverter GbkToBig5EncodingConverter { get; set; }
        public Big5ToGbkEncodingConverter Big5ToGbkEncodingConverter { get; set; }
        public UnicodeToHtmlDexCodeEncodingConverter UnicodeToHtmlDexCodeEncodingConverter { get; set; }
        public UnicodeToHtmlHexCodeEncodingConverter UnicodeToHtmlHexCodeEncodingConverter { get; set; }
        public HtmlCodeToUnicodeEncodingConverter HtmlCodeToUnicodeEncodingConverter { get; set; }
        public SymbolEncodingConverter SymbolEncodingConverter { get; set; }

        #endregion Converter

        public MainWindowService MainWindowService { get; } = new();

        public App()
        {
            #region SetupConverter

            TextConverter = new GoServiceTextConverter().StartService();

            CommonEncodingConverter = new();
            Big5ToGbkEncodingConverter = new(TextConverter);
            GbkToBig5EncodingConverter = new(TextConverter);
            UnicodeToHtmlHexCodeEncodingConverter = new();
            UnicodeToHtmlDexCodeEncodingConverter = new();
            HtmlCodeToUnicodeEncodingConverter = new();
            SymbolEncodingConverter = new();

            #endregion SetupConverter
        }

        public async Task<string> ConvertEncodingAndTextAsync(string s, Encoding encoding1, Encoding encoding2, ETextConvertMode eTextConvertMode)
        {
            if (encoding1 == encoding2)
            {
                await TextConverter.ConvertAsync(s, eTextConvertMode);
            }
            else if (encoding1 == Encoding.GetEncoding("BIG5") && encoding2 == Encoding.GetEncoding("GBK"))
            {
                s = await Big5ToGbkEncodingConverter.ConvertAsync(s);
            }
            else if (encoding1 == Encoding.GetEncoding("GBK") && encoding2 == Encoding.GetEncoding("BIG5"))
            {
                s = await GbkToBig5EncodingConverter.ConvertAsync(s);
            }
            else
            {
                s = CommonEncodingConverter.Convert(s, encoding1, encoding2);
            }
            return s;
        }

        public async Task<Dictionary<string, string>> ConvertEncodingAndTextAsync(Dictionary<string, string> dic, Encoding encoding1, Encoding encoding2, ETextConvertMode eTextConvertMode)
        {
            string json = JsonSerializer.Serialize(dic);
            json = await ConvertEncodingAndTextAsync(json, encoding1, encoding2, eTextConvertMode);
            return JsonSerializer.Deserialize<Dictionary<string, string>>(json) ?? throw new NotImplementedException();
        }

        private async void Application_Startup(object sender, StartupEventArgs e)
        {
            App.Instance = this;
            App.Reload(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "ConvertZZ.json"));
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
            MainWindowService.RegistMessenger();

            ShutdownMode = ShutdownMode.OnMainWindowClose;
            if (e.Args.Length > 0)
            {
                ITextConverter textConverter = new GoServiceTextConverter();
                IEncodingConverter encodingConverter = new LocalEncodingConverter();
                Big5ToGbkEncodingConverter big5ToGbkEncodingConverter = new(textConverter);
                GbkToBig5EncodingConverter gbkToBig5EncodingConverter = new(textConverter);
                if (e.Args[0] == "/file" || e.Args[0] == "/audio")
                {
                    var ps = Process.GetProcessesByName("ConvertZZ");
                    IntPtr hwndTest = IntPtr.Zero;
                    if (ps.Length > 1)
                    {
                        long mini = ps.ToList().Min(x => x.StartTime.Ticks);
                        hwndTest = ps.Where(x => x.StartTime.Ticks == mini).First().Handle;
                    }
                    else
                    {
                        ShowUI();
                        Window_DialogHost window_DialogHost = new(e.Args[0] == "/file" ? EMode.File_FileName : EMode.AutioTag, e.Args.Skip(1).ToArray());
                        window_DialogHost.Show();
                        return;
                    }
                    try
                    {
                        using (NamedPipeClientStream pipeClient = new(".", "ConvertZZ_Pipe", PipeDirection.InOut, PipeOptions.None, TokenImpersonationLevel.Impersonation))
                        {
                            Console.WriteLine("Connecting to server...\n");
                            await pipeClient.ConnectAsync(1000);

                            StreamString ss = new(pipeClient);
                            // The client security token is sent with the first write.
                            // Send the name of the file whose contents are returned
                            // by the server.
                            await ss.WriteStringAsync(string.Join("|", e.Args));
                            pipeClient.WaitForPipeDrain();
                            string returns = await ss.ReadStringAsync();
                            pipeClient.Close();
                        };
                    }
                    catch { }
                    Shutdown(1);
                    return;
                }
                Encoding[] encoding = new Encoding[2];
                bool EncodingSetted = false;
                ETextConvertMode eTextConvertMode = ETextConvertMode.None;
                string? path1 = null, path2 = null;
                Regex? Regex_path1 = null;
                int VocabularyCorrection = -1;
                EEngine Engine = App.Settings.Engine;
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
                            eTextConvertMode = ETextConvertMode.S2T;
                            break;

                        case "/f:s":
                            eTextConvertMode = ETextConvertMode.T2S;
                            break;

                        case "/f:d":
                            eTextConvertMode = ETextConvertMode.None;
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

                        case "/e:l":
                            Engine = EEngine.Local;
                            break;

                        case "/e:f":
                            Engine = EEngine.Fanhuaji;
                            break;

                        default:
                            if (path1 == null)
                            {
                                path1 = e.Args[i];
                                Regex_path1 = new Regex($"{Regex.Replace(path1.Replace("*", "(.*?)").Replace("\\", "\\\\"), "[\\/]", "[\\\\/]")}$");
                            }
                            else
                            {
                                path2 = e.Args[i];
                            }
                            break;
                    }
                }
                //if (VocabularyCorrection == 1 || (VocabularyCorrection == -1 && App.Settings.VocabularyCorrection))
                //{
                //    await LoadDictionary(Engine);
                //}
                string s = "";
                List<string> file = new();
                bool ModeIsOneFile = true;
                if (path1.Contains('*'))
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
                if (path1.Count(x => x == '*') != path2.Count(x => x == '*') && path2.Contains('*'))
                {
                    Console.WriteLine("參數錯誤(輸出路徑的萬用字元術與輸入路徑不同)");
                    Console.Read();
                    Shutdown(1);
                    return;
                }
                if (path1.Contains('*') && !path2.Contains('*') && File.Exists(path2))
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
                        using StreamReader streamReader = new(stream, encoding[0], false);
                        s = streamReader.ReadToEnd();
                    }
                    if (!EncodingSetted)
                        switch (EncodingAnalyzer.Analyze(s))
                        {
                            case EncodingAnalyzer.EncodingType.Ascii:
                                encoding[0] = Encoding.Default;
                                break;

                            case EncodingAnalyzer.EncodingType.BIG5:
                            case EncodingAnalyzer.EncodingType.BIG5AsGBK:
                                encoding[0] = Encoding.GetEncoding("BIG5");
                                break;

                            case EncodingAnalyzer.EncodingType.GBK:
                            case EncodingAnalyzer.EncodingType.GBKAsBIG5:
                                encoding[0] = Encoding.GetEncoding("GBK");
                                break;
                        }
                    try
                    {
                        s = await ConvertEncodingAndTextAsync(s, encoding[0], encoding[1], eTextConvertMode);
                    }
                    catch (Fanhuaji.FanhuajiException ex)
                    {
                        Console.WriteLine($"[Error][{DateTime.Now}][{ex.Message}] {f}");
                        continue;
                    }
                    if (ModeIsOneFile)
                    {
                        using StreamWriter streamWriter = new(path2, false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]);
                        streamWriter.Write(s);
                        streamWriter.Flush();
                    }
                    else
                    {
                        var m1 = Regex_path1.Match(f);
                        if (m1.Success)
                        {
                            if (path2.Contains('*'))
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
                                using StreamWriter streamWriter = new(@string, false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]);
                                streamWriter.Write(s);
                                streamWriter.Flush();
                            }
                            else
                            {
                                Directory.CreateDirectory(Path.GetDirectoryName(path2));
                                using StreamWriter streamWriter = new(Path.Combine(Path.GetDirectoryName(path2), Path.GetFileName(f)), false, encoding[1] == Encoding.UTF8 ? new UTF8Encoding(App.Settings.FileConvert.UnicodeAddBom) : encoding[1]);
                                streamWriter.Write(s);
                                streamWriter.Flush();
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
                ShowUI();
            }
        }

        private async void ShowUI()
        {
            NIcon.Icon = ConvertZZ.Properties.Resources.icon;
            NIcon.Visible = true;
            if (Settings.CheckVersion)
            {
                new Thread(new ThreadStart(() =>
                {
                    var versionReport = UpdateChecker.ChecktVersion();
                    if (versionReport != null && versionReport.HaveNew)
                    {
                        if (MessageBox.Show(String.Format("發現新版本{0}(目前版本：{1})\r\n前往官網下載更新？", versionReport.Newst.ToString(), versionReport.Current.ToString()), "發現更新", MessageBoxButton.YesNo) == MessageBoxResult.Yes)
                        {
                            Process.Start("https://github.com/flier268/ConvertZZ/releases");
                        }
                    }
                })).Start();
            }
            MainWindow = new MainWindow();
            MainWindow.Show();
        }
    }
}