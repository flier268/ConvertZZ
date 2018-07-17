using ConvertZZ.Moudle;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using static ConvertZZ.Pages.Page_AudioTags;

namespace ConvertZZ
{
    /// <summary>
    /// MainWindow.xaml 的互動邏輯
    /// </summary>
    public partial class MainWindow : Window
    {
        List<Moudle.HotKey> hotKeys = new List<Moudle.HotKey>();
        public MainWindow()
        {
            App.nIcon.MouseClick += NIcon_MouseClick;
            if (!App.Settings.AssistiveTouch)
                this.Hide();
            InitializeComponent();
            RegAllHotkey();
        }

        private void NIcon_MouseClick(object sender, System.Windows.Forms.MouseEventArgs e)
        {
            if (e.Button == System.Windows.Forms.MouseButtons.Right)
            {
                ContextMenu NotifyIconMenu = (ContextMenu)this.FindResource("NotifyIconMenu");
                NotifyIconMenu.IsOpen = true;
            }
        }

        void HotkeyAction1(Moudle.HotKey hotKey)
        {
            MenuItem_Click(new MenuItem { Uid = App.Settings.HotKey.Feature1.Action }, null);
        }
        void HotkeyAction2(Moudle.HotKey hotKey)
        {
            MenuItem_Click(new MenuItem { Uid = App.Settings.HotKey.Feature2.Action }, null);
        }
        void HotkeyAction3(Moudle.HotKey hotKey)
        {
            MenuItem_Click(new MenuItem { Uid = App.Settings.HotKey.Feature3.Action }, null);
        }
        void HotkeyAction4(Moudle.HotKey hotKey)
        {
            MenuItem_Click(new MenuItem { Uid = App.Settings.HotKey.Feature4.Action }, null);
        }
        private void RegHotkey(Feature feature, Action<Moudle.HotKey> action)
        {
            if (!feature.Enable) return;
            KeyModifier keyModifier = KeyModifier.None;
            feature.Modift.Split(',').ToList().ForEach(x => keyModifier = keyModifier | (KeyModifier)Enum.Parse(typeof(KeyModifier), x.Trim()));
            hotKeys.Add(new Moudle.HotKey((Key)Enum.Parse(typeof(Key), feature.Key), keyModifier, action));
        }
        public void RegAllHotkey()
        {
            RegHotkey(App.Settings.HotKey.Feature1, HotkeyAction1);
            RegHotkey(App.Settings.HotKey.Feature2, HotkeyAction2);
            RegHotkey(App.Settings.HotKey.Feature3, HotkeyAction3);
            RegHotkey(App.Settings.HotKey.Feature4, HotkeyAction4);
        }       
        public void UnRegAllHotkey()
        {
            hotKeys.ForEach(x => x.Dispose());
            hotKeys.Clear();
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
            UnRegAllHotkey();
            Window_Setting window_Setting = new Window_Setting() { Owner = this };
            window_Setting.ShowDialog();
            RegAllHotkey();
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
        DragDropKeyStates dragDropKeyStates;
        private void Window_DragEnter(object sender, DragEventArgs e)
        {
            //紀錄拖曳進來時的按鍵
            dragDropKeyStates = e.KeyStates;
        }
        private void Window_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                //減去輔助鍵，得到現在是左鍵還是右鍵
                dragDropKeyStates -= e.KeyStates;
                if (dragDropKeyStates == DragDropKeyStates.LeftMouseButton)
                {

                }
                else if (dragDropKeyStates == DragDropKeyStates.RightMouseButton)
                {

                }
                
                // Note that you can have more than one file.
                string[] files = (string[])e.Data.GetData(DataFormats.FileDrop);

            }
        }
        private void MenuItem_Click(object sender, RoutedEventArgs e)
        {
            string clip = ClipBoardHelper.GetClipBoard_UnicodeText();
            StringBuilder sb = new StringBuilder();
            System.Diagnostics.Stopwatch sw = new System.Diagnostics.Stopwatch();
            sw.Reset();
            sw.Start();
            switch (((MenuItem)sender).Uid)
            {
                case "1":
                    this.Visibility = this.Visibility == Visibility.Visible ? Visibility.Hidden : Visibility.Visible;
                    break;
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
                    Window_DialogHost window_File_FileNameConverter = new Window_DialogHost(  Enums.Enum_Mode.Mode.File_FileName);
                    window_File_FileNameConverter.Show();
                    break;
                case "b2":
                    Window_DialogHost window_ClipBoard_Converter = new Window_DialogHost(Enums.Enum_Mode.Mode.ClipBoard);
                    window_ClipBoard_Converter.Show();
                    break;
                case "c1":
                    Window_DialogHost Window_DialogHost = new Window_DialogHost(Enums.Enum_Mode.Mode.AutioTag,Format.ID3);
                    Window_DialogHost.Show();
                    break;
                case "c2":
                    Window_DialogHost Window_DialogHost2 = new Window_DialogHost(Enums.Enum_Mode.Mode.AutioTag, Format.APE);
                    Window_DialogHost2.Show();
                    break;
                case "c3":
                    Window_DialogHost Window_DialogHost3 = new Window_DialogHost(Enums.Enum_Mode.Mode.AutioTag, Format.OGG);
                    Window_DialogHost3.Show();
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
            //顯示提示
            switch(((MenuItem)sender).Uid)
            {
                case "1":
                case "b1":
                case "b2":
                case "c1":
                case "c2":
                case "c3":
                    break;
                default:
                    if (App.Settings.Prompt)
                    {
                        sw.Stop();
                        MessageBox.Show(this,String.Format("轉換完成\r\n耗時：{0} ms",sw.ElapsedMilliseconds));
                    }
                    break;
            }
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            UnRegAllHotkey();
        }
    }

}
