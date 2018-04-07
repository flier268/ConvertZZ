using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Web;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;

namespace ConvertZZ
{
    /// <summary>
    /// MainWindow.xaml 的互動邏輯
    /// </summary>
    public partial class MainWindow : Window
    {
        ChineseConverter chineseConverter = new ChineseConverter();
        public MainWindow()
        {
            InitializeComponent();
            foreach (string p in System.IO.Directory.GetFiles(System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Dictionary")))
                chineseConverter.Load(p);
            chineseConverter.ReloadFastReplaceDic();
        }
        Point pointNow = new Point();
        bool leftDown = false;
        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed)
            {
                pointNow = new Point(Left, Top);
                leftDown = true;
                this.DragMove();
            }
        }
        private void Setting_Click(object sender, RoutedEventArgs e)
        {
            
        }
    
        private void Exit_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }

        private void Window_MouseUp(object sender, MouseButtonEventArgs e)
        {
            if (Left == pointNow.X && Top == pointNow.Y && leftDown)
            {
                leftDown = false;
                if (Keyboard.IsKeyDown(Key.LeftCtrl) || Keyboard.IsKeyDown(Key.RightCtrl))
                    MessageBox.Show("ctrl");
                else
                    MessageBox.Show("");
            }
        }

        private void Window_Drop(object sender, DragEventArgs e)
        {

        }

        private void MenuItem_Click(object sender, RoutedEventArgs e)
        {
            string clip = ClipBoardHelper.GetClipBoard();
            switch(((MenuItem)sender).Uid)
            {
                case "a1":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("GBK"), Encoding.GetEncoding("BIG5"));
                    break;
                case "a2":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("BIG5"), Encoding.GetEncoding("GBK"));
                    break;
                case "a3":
                    clip= ChineseConverter.ToTraditional(clip);
                    if (App.Settings.VocabularyCorrection)
                        clip = chineseConverter.Convert(clip);
                    /*
                    byte[] bomBuffer = new byte[] { 0xef, 0xbb, 0xbf };

                    if (buffer[0] == bomBuffer[0]
                        && buffer[1] == bomBuffer[1]
                        && buffer[2] == bomBuffer[2])
                    {
                        return new UTF8Encoding(false).GetString(buffer, 3, buffer.Length - 3);
                    }
                    */
                    break;
                case "a4":
                    clip = ChineseConverter.ToSimplified(clip);
                    break;
                case "b1":
                    Window_TextConvertr window_TextConvertr = new Window_TextConvertr();
                    window_TextConvertr.Show();
                    break;
                case "b2":
                    Window_FileConverter window_FileConverter = new Window_FileConverter();
                    window_FileConverter.Show();
                    break;
                case "za1":
                    clip = HttpUtility.UrlEncode(clip);
                    break;
                case "za2":
                    break;
                case "zb1":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("UTF-8"), Encoding.GetEncoding("GBK"));
                    break;
                case "zb2":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("UTF-8"), Encoding.GetEncoding("BIG5"));
                    break;
                case "zb3":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("UTF-8"), Encoding.GetEncoding("shift_jis"));
                    break;
                case "zc1":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("shift_jis"), Encoding.GetEncoding("GBK"));
                    break;
                case "zc2":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("shift_jis"), Encoding.GetEncoding("BIG5"));
                    break;
                case "zc3":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("GBK"), Encoding.GetEncoding("shift_jis"));
                    break;
                case "zc4":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("BIG5"), Encoding.GetEncoding("shift_jis"));
                    break;
                case "zd1":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("hz-gb-2312"), Encoding.GetEncoding("GBK"));
                    break;
                case "zd2":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("hz-gb-2312"), Encoding.GetEncoding("BIG5"));
                    break;
                case "zd3":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("GBK"), Encoding.GetEncoding("hz-gb-2312"));
                    break;
                case "zd4":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("BIG5"), Encoding.GetEncoding("hz-gb-2312"));
                    break;
                case "ze1":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("GBK"), Encoding.GetEncoding("UFT-8"));
                    clip = ChineseConverter.ToSimplified(clip);
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("UTF-8"), Encoding.GetEncoding("GBK"));
                    break;
                case "ze2":
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("GBK"), Encoding.GetEncoding("UFT-8"));
                    clip = ChineseConverter.ToTraditional(clip);
                    if (App.Settings.VocabularyCorrection)
                        clip = chineseConverter.Convert(clip);
                    clip = TransferEncodeHelper.TransferStr(clip, Encoding.GetEncoding("UTF-8"), Encoding.GetEncoding("GBK"));
                    break;
            }
            ClipBoardHelper.SetClipBoard(clip);
        }
    }

}
