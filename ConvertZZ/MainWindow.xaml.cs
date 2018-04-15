using System;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using static ConvertZZ.Window_TagConverter;

namespace ConvertZZ
{
    /// <summary>
    /// MainWindow.xaml 的互動邏輯
    /// </summary>
    public partial class MainWindow : Window
    {
        public MainWindow()
        {
            InitializeComponent();            
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
        private void HideBall_Click(object sender, RoutedEventArgs e)
        {
            if (App.Settings.ShowBalloonTip)
                App.nIcon.ShowBalloonTip(1500, "ConvertZZ", "ConvertZZ is here", System.Windows.Forms.ToolTipIcon.Info);
            this.Hide();
        }
        private void Exit_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
            Environment.Exit(0);
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
            string clip = ClipBoardHelper.GetClipBoard_UnicodeText();
            StringBuilder sb = new StringBuilder();
            switch (((MenuItem)sender).Uid)
            {
                case "a1":
                    clip = ChineseConverter.ToTraditional(clip);
                    if (App.Settings.VocabularyCorrection)
                        clip = App.ChineseConverter.Convert(clip);
                    clip = Encoding.GetEncoding("GBK").GetString(Encoding.GetEncoding("BIG5").GetBytes(clip));
                    break;
                case "a2":                    
                    clip = ChineseConverter.ToSimplified(clip);
                    if (App.Settings.VocabularyCorrection)
                    { } //clip = chineseConverter.Convert(clip);
                    clip = Encoding.GetEncoding("BIG5").GetString(Encoding.GetEncoding("GBK").GetBytes(clip));             
                    break;
                case "a3":
                    clip = ChineseConverter.ToTraditional(clip);
                    if (App.Settings.VocabularyCorrection)
                        clip = App.ChineseConverter.Convert(clip);
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
                    if (App.Settings.VocabularyCorrection)
                        clip = App.ChineseConverter.Convert(clip);                    
                    break;
                case "b1":
                    Window_TextConvertr window_TextConvertr = new Window_TextConvertr();
                    window_TextConvertr.Show();
                    break;
                case "b2":
                    Window_FolderFileNameConverter window_FolderFileNameConverter = new Window_FolderFileNameConverter();
                    window_FolderFileNameConverter.Show();
                    break;
                case "c1":
                    Window_TagConverter window_TagConverter = new Window_TagConverter(Format.ID3);
                    window_TagConverter.Show();
                    break;
                case "c2":
                    Window_TagConverter window_TagConverter2 = new Window_TagConverter(Format.APE);
                    window_TagConverter2.Show();
                    break;
                case "c3":
                    Window_TagConverter window_TagConverter3 = new Window_TagConverter(Format.OGG);
                    window_TagConverter3.Show();
                    break;
                case "za1":                   
                    foreach(char c in clip)
                    {
                        if ((' ' <= c && c <= '~') || (c == '\r') || (c == '\n'))
                        {
                            if (c == '&')
                            {
                                sb.Append("&amp;");
                            }
                            else if(c == '<') {
                                sb.Append("&lt;");
                            }
                            else if(c == '>') {
                                sb.Append("&gt;");
                            } else {
                                sb.Append(c.ToString());
                            }
                        }
                        else
                        {
                            sb.Append("&#");
                            sb.Append(Convert.ToInt32(c));
                            sb.Append(";");
                        }
                    }
                    clip = sb.ToString();
                    break;
                case "za2":
                    foreach (char c in clip)
                    {
                        if ((' ' <= c && c <= '~') || (c == '\r') || (c == '\n'))
                        {
                            if (c == '&')
                            {
                                sb.Append("&amp;");
                            }
                            else if (c == '<')
                            {
                                sb.Append("&lt;");
                            }
                            else if (c == '>')
                            {
                                sb.Append("&gt;");
                            }
                            else
                            {
                                sb.Append(c.ToString());
                            }
                        }
                        else
                        {
                            sb.Append("&#x");
                            sb.Append(Convert.ToInt32(c).ToString("X"));
                            sb.Append(";");
                        }
                    }
                    clip = sb.ToString();
                    break;
                case "za3":
                    clip.Replace("&amp;", "&");
                    clip.Replace("&lt;", "<");
                    clip.Replace("&gt;", ">");
                    //以;將文字拆成陣列
                    string[] tmp = clip.Split(';');
                    //檢查最後一個字元是否為【;】，因為有【英文】、【阿拉伯數字】、【&#XXXX;】
                    //若最後一個要處理的字並非HTML UNICODE則不進行處理
                    bool Process_last = clip.Substring(clip.Length - 1, 1).Equals(";");
                    //Debug.WriteLine(tmp.Length + "");
                    for (int i = 0; i < tmp.Length; i++)
                    {
                        //以&#將文字拆成陣列
                        string[] tmp2 = tmp[i].Split(new string[] { "&#" }, StringSplitOptions.RemoveEmptyEntries);
                        if (tmp2.Length == 1)
                        {
                            //如果長度為1則試圖轉換UNICODE回字符，若失敗則使用原本的字元
                            if (i != tmp.Length - 1)
                            {
                                try
                                {
                                    if (tmp2[0].StartsWith("x"))
                                        sb.Append(Convert.ToChar(Convert.ToInt32(tmp2[0].Substring(1, tmp2[0].Length - 1), 16)).ToString());
                                    else
                                        sb.Append(Convert.ToChar(Convert.ToInt32(int.Parse(tmp2[0]))).ToString());
                                }
                                catch
                                {
                                    sb.Append(tmp2[0]);
                                }
                            }
                            else
                            {
                                sb.Append(tmp2[0]);
                            }
                        }
                        if (tmp2.Length == 2)
                        {
                            //若長度為2，則第一項不處理，只處理第二項即可
                            sb.Append(tmp2[0]);
                            var g = Convert.ToInt32(tmp2[1].Substring(1, tmp2[1].Length - 1), 16);
                            if (tmp2[1].StartsWith("x"))
                                sb.Append(Convert.ToChar(Convert.ToInt32(tmp2[1].Substring(1, tmp2[1].Length - 1), 16)).ToString());
                            else
                                sb.Append(Convert.ToChar(Convert.ToInt32(tmp2[1])).ToString());
                        }
                    }
                    clip = sb.ToString();
                    break;
                case "zb1":
                    //Unicode>GBK
                    clip = Encoding.Default.GetString(Encoding.GetEncoding("GBK").GetBytes(clip));
                    break;
                case "zb2":
                    clip = Encoding.Default.GetString(Encoding.GetEncoding("BIG5").GetBytes(clip));
                    break;
                case "zb3":
                    clip = Encoding.Default.GetString(Encoding.GetEncoding("Shift-JIS").GetBytes(clip));                    
                    break;
                case "zc1":
                    //Shift-JIS>GBK           
                    clip = Encoding.GetEncoding("shift_jis").GetString(Encoding.GetEncoding("GBK").GetBytes(clip));
                    break;
                case "zc2":
                    clip = Encoding.GetEncoding("shift_jis").GetString(Encoding.GetEncoding("BIG5").GetBytes(clip));
                    break;
                case "zc3":
                    clip = Encoding.GetEncoding("GBK").GetString(Encoding.GetEncoding("shift_jis").GetBytes(clip));
                    break;
                case "zc4":
                    clip = Encoding.GetEncoding("BIG5").GetString(Encoding.GetEncoding("shift_jis").GetBytes(clip));
                    break;
                case "zd1":
                    //hz-gb-2312>GBK
                    clip = Encoding.GetEncoding("hz-gb-2312").GetString(Encoding.GetEncoding("GBK").GetBytes(clip));
                    break;
                case "zd2":
                    clip = Encoding.GetEncoding("hz-gb-2312").GetString(Encoding.GetEncoding("BIG5").GetBytes(clip));
                    break;
                case "zd3":
                    clip = Encoding.GetEncoding("GBK").GetString(Encoding.GetEncoding("hz-gb-2312").GetBytes(clip));
                    break;
                case "zd4":
                    clip = Encoding.GetEncoding("BIG5").GetString(Encoding.GetEncoding("hz-gb-2312").GetBytes(clip));
                    break;               
            }
            ClipBoardHelper.SetClipBoard_UnicodeText(clip);
            //ClipBoardHelper.SetClipBoard_ByteArray(clip);
        }
    }

}
