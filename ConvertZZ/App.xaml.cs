using ConvertZZ.Moudle;
using System;
using System.Diagnostics;
using System.IO;
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
            App.Reload(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"ConvertZZ.json"));
            Task.Run(() => {
                foreach (string p in System.IO.Directory.GetFiles(System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Dictionary")))
                    ChineseConverter.Load(p);
                ChineseConverter.ReloadFastReplaceDic();
                DicLoaded = true;
            });
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
        }

        public static ChineseConverter ChineseConverter { get; set; } = new ChineseConverter();
    }
}
