using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Interop;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Moudle;
using static Fanhuaji_API.Fanhuaji;

namespace ConvertZZ.Views.Pages
{
    /// <summary>
    /// Page_ClipBoard.xaml 的互動邏輯
    /// </summary>
    public partial class Page_ClipBoard : Page, INotifyPropertyChanged
    {
        private IntPtr hwnd = IntPtr.Zero;

        public Page_ClipBoard(IntPtr hwnd)
        {
            this.hwnd = hwnd;
            InitializeComponent();
            DataContext = this;
        }

        public string ClipBoard { get; set; }

        public string Output { get; set; }

        /// <summary>
        /// 編碼轉換 [0]:來源編碼   [1]:輸出編碼
        /// </summary>
        private Encoding[] encoding = new Encoding[2] { Encoding.GetEncoding("BIG5"), Encoding.GetEncoding("GBK") };

        /// <summary>
        /// 輸出簡繁轉換：0:一般  1:繁體中文 2:簡體中文
        /// </summary>
        private ETextConvertMode ToChinese = ETextConvertMode.None;

        private async void Encoding_Selected(object sender, RoutedEventArgs e)
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
            await Preview();
        }

        private async void Chinese_Click(object sender, RoutedEventArgs e)
        {
            switch (((RadioButton)sender).Uid)
            {
                case "NChinese":
                    ToChinese = 0;
                    break;

                case "TChinese":
                    ToChinese = ETextConvertMode.S2T;
                    break;

                case "CChinese":
                    ToChinese = ETextConvertMode.T2S;
                    break;
            }
            await Preview();
        }

        private async Task Preview()
        {
            try
            {
                Output = await App.Instance.ConvertEncodingAndTextAsync(ClipBoard, encoding[0], encoding[1], ToChinese);
            }
            catch (FanhuajiException val)
            {
                FanhuajiException fe = val;
                Window_MessageBoxEx.ShowDialog(((Exception)fe).Message, "繁化姬API", "確定");
            }
        }

        public event PropertyChangedEventHandler PropertyChanged;

        private void Page_Loaded(object sender, RoutedEventArgs e)
        {
            #region 註冊Hook並監聽剪貼簿

            hWndSource = HwndSource.FromHwnd(hwnd);
            hWndSource.AddHook(this.WinProc);   // start processing window messages
            mNextClipBoardViewerHWnd = SetClipboardViewer(hWndSource.Handle);   // set this window as a viewer

            #endregion 註冊Hook並監聽剪貼簿
        }

        private void Page_Unloaded(object sender, RoutedEventArgs e)
        {
            ChangeClipboardChain(hWndSource.Handle, mNextClipBoardViewerHWnd);
        }

        private IntPtr WinProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            switch (msg)
            {
                case WM_CHANGECBCHAIN:
                    if (wParam == mNextClipBoardViewerHWnd)
                    {
                        // clipboard viewer chain changed, need to fix it.
                        mNextClipBoardViewerHWnd = lParam;
                    }
                    else if (mNextClipBoardViewerHWnd != IntPtr.Zero)
                    {
                        // pass the message to the next viewer.
                        SendMessage(mNextClipBoardViewerHWnd, msg, wParam, lParam);
                    }
                    break;

                case WM_DRAWCLIPBOARD:
                    // clipboard content changed
                    if (Clipboard.ContainsText())
                    {
                        ClipBoard = ClipBoardHelper.GetClipBoard_UnicodeText();
                        ((ThreadStart)async delegate
                        {
                            await Preview();
                        })();
                    }

                    // pass the message to the next viewer.
                    SendMessage(mNextClipBoardViewerHWnd, msg, wParam, lParam);
                    break;
            }

            return IntPtr.Zero;
        }

        private HwndSource hWndSource;

        #region Definitions

        //Constants for API Calls...
        private const int WM_DRAWCLIPBOARD = 0x308;

        private const int WM_CHANGECBCHAIN = 0x30D;

        //Handle for next clipboard viewer...
        private IntPtr mNextClipBoardViewerHWnd;

        //API declarations...
        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        private static extern IntPtr SetClipboardViewer(IntPtr hWndNewViewer);

        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        private static extern bool ChangeClipboardChain(IntPtr HWnd, IntPtr HWndNext);

        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        public static extern int SendMessage(IntPtr hWnd, int msg, IntPtr wParam, IntPtr lParam);

        #endregion Definitions

        private void Button_CopyOutput_Click(object sender, RoutedEventArgs e)
        {
            ClipBoardHelper.SetClipBoard_UnicodeText(Output);
        }
    }
}